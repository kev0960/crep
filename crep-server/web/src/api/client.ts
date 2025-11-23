import type {
  SearchMode,
  SearchRequest,
  SearchResponse,
  ErrorResponse,
} from "./types";

type SearchOptions = {
  mode?: SearchMode;
  limit?: number;
};

export async function executeSearch(
  query: string,
  options: SearchOptions = {},
): Promise<SearchResponse> {
  const trimmed = query.trim();
  if (!trimmed) {
    throw new Error("Please enter a search query.");
  }

  const payload: SearchRequest = {
    query: trimmed,
    page: 0,
    page_size: 100,
  };

  if (options.mode) {
    payload.mode = options.mode;
  }

  const response = await fetch("/api/search", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(payload),
  });

  if (!response.ok) {
    let message = `Request failed with status ${response.status}`;

    try {
      const body = (await response.json()) as Partial<ErrorResponse>;
      if (body?.message) {
        message = body.message;
      }
    } catch {
      // Ignore JSON parse failures – fall back to status text.
    }

    throw new Error(message);
  }

  return (await response.json()) as SearchResponse;
}
