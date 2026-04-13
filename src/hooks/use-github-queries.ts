import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type Dispatch,
  type SetStateAction,
} from "react";
import {
  skipToken,
  useQueries,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import type { ReviewThread } from "../lib/review-threads";
import {
  githubKeys,
  initialReposQueryOptions,
  pullRequestCachedListQueryOptions,
  pullRequestFilesQueryOptions,
  pullRequestListQueryOptions,
  pullRequestPatchQueryOptions,
  pullRequestReviewThreadsQueryOptions,
  savedReposQueryOptions,
  searchReposQueryOptions,
} from "../queries/github";
import type {
  PrPatch,
  PullRequestSummary,
  RepoSummary,
  SelectedPullRequest,
} from "../types/github";

function getErrorMessage(error: unknown): string {
  if (!error) return "";
  if (error instanceof Error) return error.message;
  return String(error);
}

function useSavedRepos() {
  const query = useQuery(savedReposQueryOptions());
  return {
    ...query,
    repos: query.data ?? [],
  };
}

function useRepoPickerRepos(debouncedQuery: string) {
  const queryClient = useQueryClient();
  const trimmedQuery = debouncedQuery.trim();

  const { data: initialRepos = [], isPending: isInitialLoading } = useQuery(
    initialReposQueryOptions(),
  );

  const {
    data: searchRepos = [],
    error: searchError,
    isPending: isSearchLoading,
  } = useQuery({
    ...searchReposQueryOptions(debouncedQuery),
    enabled: trimmedQuery.length > 0,
  });

  useEffect(() => {
    void queryClient.prefetchQuery(initialReposQueryOptions());
  }, [queryClient]);

  const availableRepos = trimmedQuery.length > 0 ? searchRepos : initialRepos;
  const isLoadingRepos = trimmedQuery.length > 0 ? isSearchLoading : isInitialLoading;

  return {
    availableRepos,
    availableReposError: searchError,
    isLoadingRepos,
  };
}

type UseRepoPullRequestsArgs = {
  repos: RepoSummary[];
  setSelectedPr: Dispatch<SetStateAction<SelectedPullRequest | null>>;
};

function useRepoPullRequests({
  repos,
  setSelectedPr,
}: UseRepoPullRequestsArgs) {
  const queryClient = useQueryClient();
  const [loadingRepos, setLoadingRepos] = useState<Record<string, boolean>>({});
  const [repoErrors, setRepoErrors] = useState<Record<string, string>>({});

  const repoNames = useMemo(
    () => repos.map((repo) => repo.nameWithOwner),
    [repos],
  );

  const pullRequestQueries = useQueries({
    queries: repoNames.map((repo) => ({
      ...pullRequestListQueryOptions(repo),
      enabled: false,
    })),
  });

  const prsByRepo = useMemo(() => {
    const entries: Array<[string, PullRequestSummary[]]> = [];
    for (let i = 0; i < repoNames.length; i += 1) {
      const repo = repoNames[i];
      const pullRequests = pullRequestQueries[i]?.data;
      if (!pullRequests) continue;
      entries.push([repo, pullRequests]);
    }
    return Object.fromEntries(entries);
  }, [repoNames, pullRequestQueries]);

  const loadPullRequests = useCallback(
    async (repo: string) => {
      const listOptions = pullRequestListQueryOptions(repo);
      const existingPullRequests =
        queryClient.getQueryData<PullRequestSummary[]>(listOptions.queryKey) ?? [];
      let hasVisibleData = existingPullRequests.length > 0;

      setLoadingRepos((current) => ({ ...current, [repo]: true }));
      setRepoErrors((current) => ({ ...current, [repo]: "" }));

      try {
        const cachedPullRequests = await queryClient.fetchQuery(
          pullRequestCachedListQueryOptions(repo),
        );

        if (cachedPullRequests.length > 0 || existingPullRequests.length === 0) {
          queryClient.setQueryData(listOptions.queryKey, cachedPullRequests);
        }

        hasVisibleData = hasVisibleData || cachedPullRequests.length > 0;
      } catch (error) {
        if (!hasVisibleData) {
          setRepoErrors((current) => ({
            ...current,
            [repo]: getErrorMessage(error),
          }));
        }
      }

      try {
        const pullRequests = await queryClient.fetchQuery(listOptions);

        setSelectedPr((current) => {
          if (!current || current.repo !== repo) return current;
          const refreshedSelection = pullRequests.find(
            (pullRequest) => pullRequest.number === current.number,
          );

          if (
            !refreshedSelection ||
            refreshedSelection.headSha === current.headSha
          ) {
            return current;
          }

          return {
            ...current,
            headSha: refreshedSelection.headSha,
          };
        });

        hasVisibleData = true;
      } catch (error) {
        if (!hasVisibleData) {
          setRepoErrors((current) => ({
            ...current,
            [repo]: getErrorMessage(error),
          }));
        }
      } finally {
        setLoadingRepos((current) => ({ ...current, [repo]: false }));
      }
    },
    [queryClient, setSelectedPr],
  );

  return {
    loadingRepos,
    loadPullRequests,
    prsByRepo,
    repoErrors,
  };
}

function useSelectedPullRequestData(selectedPr: SelectedPullRequest | null) {
  const selectedPatchQuery = useQuery(
    selectedPr
      ? pullRequestPatchQueryOptions(selectedPr)
      : {
          queryKey: githubKeys.pullRequestPatchIdle(),
          queryFn: skipToken,
        },
  );

  const changedFilesQuery = useQuery(
    selectedPr
      ? pullRequestFilesQueryOptions(selectedPr)
      : {
          queryKey: githubKeys.pullRequestFilesIdle(),
          queryFn: skipToken,
        },
  );

  const reviewThreadsQuery = useQuery(
    selectedPr
      ? pullRequestReviewThreadsQueryOptions(selectedPr)
      : {
          queryKey: githubKeys.pullRequestReviewThreadsIdle(),
          queryFn: skipToken,
        },
  );

  const selectedPatch = (selectedPatchQuery.data as PrPatch | undefined) ?? null;
  const changedFiles = (changedFilesQuery.data as string[] | undefined) ?? [];
  const reviewThreads =
    (reviewThreadsQuery.data as ReviewThread[] | undefined) ?? [];

  const isPatchLoading =
    selectedPr !== null &&
    (selectedPatchQuery.isPending ||
      (selectedPatchQuery.isFetching && !selectedPatchQuery.data));
  const isChangedFilesLoading =
    selectedPr !== null &&
    (changedFilesQuery.isPending ||
      (changedFilesQuery.isFetching && !changedFilesQuery.data));
  const isReviewThreadsLoading =
    selectedPr !== null &&
    (reviewThreadsQuery.isPending ||
      (reviewThreadsQuery.isFetching && !reviewThreadsQuery.data));

  return {
    changedFiles,
    changedFilesError: getErrorMessage(changedFilesQuery.error),
    isChangedFilesLoading,
    isPatchLoading,
    isReviewThreadsLoading,
    patchError: getErrorMessage(selectedPatchQuery.error),
    reviewThreads,
    reviewThreadsError: getErrorMessage(reviewThreadsQuery.error),
    selectedPatch,
  };
}

export {
  useRepoPickerRepos,
  useRepoPullRequests,
  useSavedRepos,
  useSelectedPullRequestData,
};
