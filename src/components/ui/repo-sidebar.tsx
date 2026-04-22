import { PlusIcon } from "@heroicons/react/20/solid";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Accordion } from "./accordion";
import { RepoSidebarItem, type PullRequestSummary } from "./repo-sidebar-item";
import type { RepoSummary } from "../../types/github";

type RepoSidebarProps = {
  repos: RepoSummary[];
  prsByRepo: Record<string, PullRequestSummary[]>;
  loadingRepos: Record<string, boolean>;
  refreshingRepos: Record<string, boolean>;
  repoErrors: Record<string, string>;
  defaultOpenValues: string[];
  onAddRepo: () => void;
  onSelectPr: (repo: string, pullRequest: PullRequestSummary) => void;
  onRepoOpenChange: (repo: string, open: boolean) => void;
};

function RepoSidebar({
  repos,
  prsByRepo,
  loadingRepos,
  refreshingRepos,
  repoErrors,
  defaultOpenValues,
  onAddRepo,
  onSelectPr,
  onRepoOpenChange,
}: RepoSidebarProps) {
  const appWindow = getCurrentWindow();

  return (
    <aside className="flex h-full min-h-0 min-w-0 flex-col overflow-hidden border-ink-300 bg-canvas md:border-b-0">
      <div
        aria-hidden="true"
        className="h-8 shrink-0 cursor-grab bg-canvas active:cursor-grabbing"
        data-tauri-drag-region
        onMouseDown={(event) => {
          if (event.button !== 0) return;
          if (event.detail === 2) {
            void appWindow.toggleMaximize();
            return;
          }
          void appWindow.startDragging();
        }}
      />
      <div className="sticky top-0 z-10 flex w-full items-center gap-2.5 bg-canvas px-3 py-2.5 text-sm font-medium">
        Repositories
        <button
          aria-label="Add repo"
          className="ml-auto inline-flex items-center justify-center rounded-md p-1 text-neutral-500 transition hover:bg-canvasDark hover:text-ink-700"
          onClick={onAddRepo}
          type="button"
        >
          <PlusIcon className="size-5 shrink-0" />
        </button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        {repos.length === 0 ? (
          <div className="flex min-h-[180px] flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-ink-300 bg-surface px-6 py-8 text-center">
            <strong>No repos yet.</strong>
            <span className="text-sm text-ink-600">
              Click + to pick from your GitHub repos.
            </span>
          </div>
        ) : (
          <Accordion multiple defaultValue={defaultOpenValues}>
            {repos.map((repo) => (
              <RepoSidebarItem
                key={repo.nameWithOwner}
                value={repo.nameWithOwner}
                nameWithOwner={repo.nameWithOwner}
                pullRequests={prsByRepo[repo.nameWithOwner]}
                isLoading={Boolean(loadingRepos[repo.nameWithOwner])}
                isRefreshing={Boolean(refreshingRepos[repo.nameWithOwner])}
                error={repoErrors[repo.nameWithOwner]}
                onSelectPr={(name, pr) => onSelectPr(name, pr)}
                onOpenChange={(open) =>
                  onRepoOpenChange(repo.nameWithOwner, open)
                }
              />
            ))}
          </Accordion>
        )}
      </div>
    </aside>
  );
}

export { RepoSidebar };
export type { RepoSidebarProps, RepoSummary };
