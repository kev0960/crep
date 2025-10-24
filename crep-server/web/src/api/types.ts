/**
 * These types are kept in sync with the Rust OpenAPI schema exposed by the
 * Axum server (`/docs.ts`). They intentionally mirror the structures emitted
 * by `crep_server::api::search`.
 */
export type SearchMode = "plain" | "regex";

export interface SearchRequest {
  query: string;
  mode?: SearchMode;
  limit?: number;
}

export interface SearchResponse {
  results: SearchHit[];
}

export interface SearchHit {
  file_path: string;
  first_match: MatchDetail;
  last_match: MatchDetail | null;
}

export interface MatchDetail {
  commit_index: number;
  commit_sha: string;
  commit_date: string;
  commit_summary: string;
  lines: LineMatch[];
}

export interface LineMatch {
  line_number: number;
  content: string;
  highlights: LineHighlight[];
}

export interface LineHighlight {
  term: string;
  column: number;
}

export interface ErrorResponse {
  message: string;
}
