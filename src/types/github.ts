import type { GitStatusEntry } from "@pierre/trees";

type RepoSummary = {
  name: string;
  nameWithOwner: string;
  description: string | null;
  isPrivate: boolean | null;
};

type PullRequestSummary = {
  number: number;
  title: string;
  state: string;
  authorLogin: string;
  updatedAt: string;
  url: string;
  headSha: string;
  baseSha: string | null;
};

type SelectedPullRequest = {
  repo: string;
  number: number;
  headSha: string;
};

type PrPatch = {
  repo: string;
  number: number;
  headSha: string;
  patch: string;
};

type ViewerLogin = {
  login: string;
};

type ReviewCommentSide = "LEFT" | "RIGHT";

type CreatePullRequestReviewCommentInput = {
  repo: string;
  number: number;
  headSha: string;
  body: string;
  path: string;
  line: number | null;
  side: ReviewCommentSide | null;
  startLine: number | null;
  startSide: ReviewCommentSide | null;
  subjectType: "file" | "line";
};

type ReplyToPullRequestReviewCommentInput = {
  repo: string;
  number: number;
  commentId: number;
  body: string;
};

type UpdatePullRequestReviewCommentInput = {
  repo: string;
  commentId: number;
  body: string;
};

type FileStatsEntry = {
  additions: number;
  deletions: number;
  status: GitStatusEntry["status"];
};

export type {
  CreatePullRequestReviewCommentInput,
  FileStatsEntry,
  PrPatch,
  PullRequestSummary,
  ReplyToPullRequestReviewCommentInput,
  RepoSummary,
  ReviewCommentSide,
  SelectedPullRequest,
  UpdatePullRequestReviewCommentInput,
  ViewerLogin,
};
