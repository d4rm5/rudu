use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use rusqlite::{params, Connection, OptionalExtension};

use crate::models::{PullRequestSummary, RepoSummary};
use crate::support::{bool_to_sql, now_unix_timestamp, sql_to_bool};

static CACHE_DB_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn cache_db_path() -> Result<&'static PathBuf, String> {
    CACHE_DB_PATH
        .get()
        .ok_or_else(|| "Cache database path is not initialized".to_string())
}

pub fn set_cache_db_path(path: PathBuf) -> Result<(), PathBuf> {
    CACHE_DB_PATH.set(path)
}

pub fn open_cache_connection() -> Result<Connection, String> {
    let path = cache_db_path()?;
    Connection::open(path).map_err(|error| {
        format!(
            "Failed to open cache database at {}: {error}",
            path.display()
        )
    })
}

pub fn initialize_cache_database(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create cache directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let conn = Connection::open(path).map_err(|error| {
        format!(
            "Failed to initialize cache database at {}: {error}",
            path.display()
        )
    })?;

    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;

        CREATE TABLE IF NOT EXISTS repos (
            name_with_owner TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            is_private INTEGER,
            added_at INTEGER NOT NULL,
            last_opened_at INTEGER
        );

        CREATE TABLE IF NOT EXISTS repo_pull_requests (
            repo_name_with_owner TEXT NOT NULL,
            pr_number INTEGER NOT NULL,
            title TEXT NOT NULL,
            state TEXT NOT NULL,
            author_login TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            url TEXT NOT NULL,
            head_sha TEXT NOT NULL,
            base_sha TEXT,
            cached_at INTEGER NOT NULL,
            last_seen_at INTEGER NOT NULL,
            PRIMARY KEY (repo_name_with_owner, pr_number)
        );

        CREATE INDEX IF NOT EXISTS idx_repo_pull_requests_repo_updated
            ON repo_pull_requests (repo_name_with_owner, updated_at DESC);

        CREATE TABLE IF NOT EXISTS pr_patch_cache (
            repo_name_with_owner TEXT NOT NULL,
            pr_number INTEGER NOT NULL,
            head_sha TEXT NOT NULL,
            patch_text TEXT NOT NULL,
            cached_at INTEGER NOT NULL,
            last_accessed_at INTEGER NOT NULL,
            PRIMARY KEY (repo_name_with_owner, pr_number, head_sha)
        );

        CREATE TABLE IF NOT EXISTS pr_changed_files_cache (
            repo_name_with_owner TEXT NOT NULL,
            pr_number INTEGER NOT NULL,
            head_sha TEXT NOT NULL,
            files_json TEXT NOT NULL,
            cached_at INTEGER NOT NULL,
            last_accessed_at INTEGER NOT NULL,
            PRIMARY KEY (repo_name_with_owner, pr_number, head_sha)
        );
        ",
    )
    .map_err(|error| format!("Failed to initialize cache schema: {error}"))?;

    Ok(())
}

pub fn read_cached_pull_requests(repo: &str) -> Result<Vec<PullRequestSummary>, String> {
    let conn = open_cache_connection()?;
    let mut statement = conn
        .prepare(
            "
            SELECT
                pr_number,
                title,
                state,
                author_login,
                updated_at,
                url,
                head_sha,
                base_sha
            FROM repo_pull_requests
            WHERE repo_name_with_owner = ?1
            ORDER BY updated_at DESC
            ",
        )
        .map_err(|error| format!("Failed to prepare cached pull requests query: {error}"))?;

    let rows = statement
        .query_map(params![repo], |row| {
            Ok(PullRequestSummary {
                number: row.get(0)?,
                title: row.get(1)?,
                state: row.get(2)?,
                author_login: row.get(3)?,
                updated_at: row.get(4)?,
                url: row.get(5)?,
                head_sha: row.get(6)?,
                base_sha: row.get(7)?,
            })
        })
        .map_err(|error| format!("Failed to read cached pull requests: {error}"))?;

    let mut results = Vec::new();
    for row in rows {
        results.push(
            row.map_err(|error| format!("Failed to parse cached pull request row: {error}"))?,
        );
    }

    Ok(results)
}

pub fn write_pull_requests_cache(
    repo: &str,
    pull_requests: &[PullRequestSummary],
) -> Result<(), String> {
    let mut conn = open_cache_connection()?;
    let tx = conn
        .transaction()
        .map_err(|error| format!("Failed to start pull request cache transaction: {error}"))?;

    tx.execute(
        "DELETE FROM repo_pull_requests WHERE repo_name_with_owner = ?1",
        params![repo],
    )
    .map_err(|error| format!("Failed to clear cached pull requests: {error}"))?;

    let timestamp = now_unix_timestamp();

    for pull_request in pull_requests {
        tx.execute(
            "
            INSERT INTO repo_pull_requests (
                repo_name_with_owner,
                pr_number,
                title,
                state,
                author_login,
                updated_at,
                url,
                head_sha,
                base_sha,
                cached_at,
                last_seen_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
            ",
            params![
                repo,
                pull_request.number,
                pull_request.title,
                pull_request.state,
                pull_request.author_login,
                pull_request.updated_at,
                pull_request.url,
                pull_request.head_sha,
                pull_request.base_sha,
                timestamp,
            ],
        )
        .map_err(|error| {
            format!(
                "Failed to cache pull request {}: {error}",
                pull_request.number
            )
        })?;
    }

    tx.commit()
        .map_err(|error| format!("Failed to commit pull request cache transaction: {error}"))
}

pub fn get_cached_patch(repo: &str, number: u32, head_sha: &str) -> Result<Option<String>, String> {
    let conn = open_cache_connection()?;
    let patch = conn
        .query_row(
            "
            SELECT patch_text
            FROM pr_patch_cache
            WHERE repo_name_with_owner = ?1
              AND pr_number = ?2
              AND head_sha = ?3
            ",
            params![repo, number, head_sha],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("Failed to query cached patch: {error}"))?;

    if patch.is_some() {
        conn.execute(
            "
            UPDATE pr_patch_cache
            SET last_accessed_at = ?4
            WHERE repo_name_with_owner = ?1
              AND pr_number = ?2
              AND head_sha = ?3
            ",
            params![repo, number, head_sha, now_unix_timestamp()],
        )
        .map_err(|error| format!("Failed to update patch cache access time: {error}"))?;
    }

    Ok(patch)
}

pub fn store_patch(repo: &str, number: u32, head_sha: &str, patch: &str) -> Result<(), String> {
    let conn = open_cache_connection()?;
    let timestamp = now_unix_timestamp();
    conn.execute(
        "
        INSERT INTO pr_patch_cache (
            repo_name_with_owner,
            pr_number,
            head_sha,
            patch_text,
            cached_at,
            last_accessed_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?5)
        ON CONFLICT(repo_name_with_owner, pr_number, head_sha)
        DO UPDATE SET
            patch_text = excluded.patch_text,
            cached_at = excluded.cached_at,
            last_accessed_at = excluded.last_accessed_at
        ",
        params![repo, number, head_sha, patch, timestamp],
    )
    .map_err(|error| format!("Failed to persist patch cache: {error}"))?;

    Ok(())
}

pub fn get_cached_changed_files(
    repo: &str,
    number: u32,
    head_sha: &str,
) -> Result<Option<Vec<String>>, String> {
    let conn = open_cache_connection()?;
    let files_json = conn
        .query_row(
            "
            SELECT files_json
            FROM pr_changed_files_cache
            WHERE repo_name_with_owner = ?1
              AND pr_number = ?2
              AND head_sha = ?3
            ",
            params![repo, number, head_sha],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("Failed to query cached changed files: {error}"))?;

    let Some(files_json) = files_json else {
        return Ok(None);
    };

    conn.execute(
        "
        UPDATE pr_changed_files_cache
        SET last_accessed_at = ?4
        WHERE repo_name_with_owner = ?1
          AND pr_number = ?2
          AND head_sha = ?3
        ",
        params![repo, number, head_sha, now_unix_timestamp()],
    )
    .map_err(|error| format!("Failed to update changed files cache access time: {error}"))?;

    let files = serde_json::from_str::<Vec<String>>(&files_json)
        .map_err(|error| format!("Failed to parse cached changed files: {error}"))?;

    Ok(Some(files))
}

pub fn store_changed_files(
    repo: &str,
    number: u32,
    head_sha: &str,
    files: &[String],
) -> Result<(), String> {
    let conn = open_cache_connection()?;
    let files_json = serde_json::to_string(files)
        .map_err(|error| format!("Failed to serialize changed files for cache: {error}"))?;
    let timestamp = now_unix_timestamp();

    conn.execute(
        "
        INSERT INTO pr_changed_files_cache (
            repo_name_with_owner,
            pr_number,
            head_sha,
            files_json,
            cached_at,
            last_accessed_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?5)
        ON CONFLICT(repo_name_with_owner, pr_number, head_sha)
        DO UPDATE SET
            files_json = excluded.files_json,
            cached_at = excluded.cached_at,
            last_accessed_at = excluded.last_accessed_at
        ",
        params![repo, number, head_sha, files_json, timestamp],
    )
    .map_err(|error| format!("Failed to persist changed files cache: {error}"))?;

    Ok(())
}

pub fn update_repo_access_timestamp(repo: &str) -> Result<(), String> {
    let conn = open_cache_connection()?;
    conn.execute(
        "
        UPDATE repos
        SET last_opened_at = ?2
        WHERE name_with_owner = ?1
        ",
        params![repo, now_unix_timestamp()],
    )
    .map_err(|error| format!("Failed to update repo access timestamp: {error}"))?;

    Ok(())
}

pub fn read_saved_repos() -> Result<Vec<RepoSummary>, String> {
    let conn = open_cache_connection()?;
    let mut statement = conn
        .prepare(
            "
            SELECT name, name_with_owner, description, is_private
            FROM repos
            ORDER BY added_at ASC
            ",
        )
        .map_err(|error| format!("Failed to prepare saved repos query: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(RepoSummary {
                name: row.get(0)?,
                name_with_owner: row.get(1)?,
                description: row.get(2)?,
                is_private: sql_to_bool(row.get(3)?),
            })
        })
        .map_err(|error| format!("Failed to load saved repos: {error}"))?;

    let mut repos = Vec::new();
    for row in rows {
        repos.push(row.map_err(|error| format!("Failed to parse saved repo row: {error}"))?);
    }

    Ok(repos)
}

pub fn save_repo_to_cache(repo: &RepoSummary) -> Result<(), String> {
    let conn = open_cache_connection()?;
    let timestamp = now_unix_timestamp();

    conn.execute(
        "
        INSERT INTO repos (
            name,
            name_with_owner,
            description,
            is_private,
            added_at,
            last_opened_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?5)
        ON CONFLICT(name_with_owner)
        DO UPDATE SET
            name = excluded.name,
            description = excluded.description,
            is_private = excluded.is_private
        ",
        params![
            &repo.name,
            &repo.name_with_owner,
            &repo.description,
            bool_to_sql(repo.is_private),
            timestamp,
        ],
    )
    .map_err(|error| format!("Failed to save repo {}: {error}", repo.name_with_owner))?;

    Ok(())
}

pub fn fetch_pull_requests_from_github(repo: &str) -> Result<Vec<PullRequestSummary>, String> {
    let stdout = crate::github::run_gh(&[
        "pr",
        "list",
        "-R",
        repo,
        "--state",
        "open",
        "--limit",
        "100",
        "--json",
        "number,title,state,author,updatedAt,url,headRefOid,baseRefOid",
    ])?;

    let pull_requests = serde_json::from_str::<Vec<crate::models::GhPullRequest>>(&stdout)
        .map_err(|error| format!("Failed to parse pull requests: {error}"))?;

    Ok(pull_requests
        .into_iter()
        .map(|pull_request| PullRequestSummary {
            number: pull_request.number,
            title: pull_request.title,
            state: pull_request.state,
            author_login: pull_request
                .author
                .map(|author| author.login)
                .unwrap_or_else(|| "unknown".into()),
            updated_at: pull_request.updated_at,
            url: pull_request.url,
            head_sha: pull_request.head_ref_oid,
            base_sha: pull_request.base_ref_oid,
        })
        .collect())
}
