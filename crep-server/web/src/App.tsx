import type { FormEvent, ReactNode } from "react";
import { useState } from "react";
import { executeSearch } from "./api/client";
import type {
  LineMatch,
  MatchDetail,
  SearchHit,
  SearchMode,
} from "./api/types";
import "./App.css";

const CREP_ASCII = `
   _____ _____  ______ _____   
  / ____|  __ \|  ____|  __ \  
 | |    | |__) | |__  | |__) | 
 | |    |  _  /|  __| |  ___/  
| |____| | \ \| |____| |      
 \_____|_|  \_\______|_|      
`;

const highlightLine = (line: LineMatch) => {
  if (!line.highlights.length) {
    return line.content;
  }

  const sorted = [...line.highlights].sort((a, b) => a.column - b.column);

  const segments: ReactNode[] = [];
  let cursor = 0;

  sorted.forEach((highlight, index) => {
    const start = Math.max(0, highlight.column);
    const termLength = highlight.term.length;
    const end = Math.min(start + termLength, line.content.length);

    if (start > cursor) {
      segments.push(
        <span key={`text-${line.line_number}-${index}`}>
          {line.content.slice(cursor, start)}
        </span>,
      );
    }

    const highlighted = line.content.slice(start, end) || highlight.term;

    segments.push(
      <mark key={`mark-${line.line_number}-${index}`}>{highlighted}</mark>,
    );

    cursor = Math.max(cursor, end);
  });

  if (cursor < line.content.length) {
    segments.push(
      <span key={`tail-${line.line_number}`}>
        {line.content.slice(cursor)}
      </span>,
    );
  }

  return segments;
};

const renderMatchSection = (
  label: string,
  detail: MatchDetail | null | undefined,
) => {
  if (!detail) {
    return null;
  }

  const date = new Date(detail.commit_date);
  const formattedDate = Number.isNaN(date.getTime())
    ? detail.commit_date
    : date.toLocaleString();

  return (
    <article className="mt-4 rounded-xl border border-black/10 bg-white px-5 py-4 shadow-sm">
      <header className="mb-2">
        <strong>{label}</strong>
        <div className="mt-1 flex flex-wrap gap-3 text-sm text-[#5f6368]">
          <span>
            Commit: <code>{detail.commit_sha.substring(0, 7)}</code>
          </span>
          <span>When: {formattedDate}</span>
        </div>
        {detail.commit_summary && (
          <p className="mt-2 text-[#3c4043]">
            {detail.commit_summary}
          </p>
        )}
      </header>

      <div className="overflow-x-auto rounded-lg bg-[#f1f3f4] p-3 font-mono text-[0.95rem]">
        {detail.lines.map((line) => (
          <div
            key={`${detail.commit_sha}-${line.line_number}`}
            className="flex items-start gap-4"
          >
            <span className="min-w-[3rem] text-right text-[#9aa0a6]">
              {line.line_number}
            </span>
            <code className="whitespace-pre-wrap">{highlightLine(line)}</code>
          </div>
        ))}
      </div>
    </article>
  );
};

function App() {
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<SearchMode>("plain");
  const [results, setResults] = useState<SearchHit[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hasSubmitted, setHasSubmitted] = useState(false);

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setHasSubmitted(true);
    setLoading(true);
    setError(null);

    try {
      const response = await executeSearch(query, { mode });
      setResults(response.results);
    } catch (err) {
      setResults([]);
      setError(err instanceof Error ? err.message : "Unknown search error.");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen bg-[#f8f9fa] font-sans text-[#202124]">
      <div
        className={`flex flex-col items-center px-6 transition-all duration-300 ${hasSubmitted ? "pt-12" : "pt-[18vh]"}`}
      >
        <pre
          className={`mb-8 text-center font-mono tracking-[0.05rem] text-[#202124] ${hasSubmitted ? "text-base" : "text-[1.3rem]"}`}
        >
          {CREP_ASCII}
        </pre>

        <form onSubmit={handleSubmit} className="w-full">
          <div className="mx-auto flex w-full max-w-[600px] items-center gap-3 rounded-full bg-white px-5 py-3 shadow-[0_1px_4px_rgba(0,0,0,0.06),0_2px_8px_rgba(0,0,0,0.04)]">
            <input
              type="text"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search git history…"
              className="flex-1 border-0 bg-transparent text-[1.05rem] focus:outline-none"
              aria-label="Search query"
            />
            <button
              type="submit"
              className="rounded-full border-0 bg-[#1a73e8] px-5 py-2 text-[0.95rem] font-semibold text-white transition-colors hover:bg-[#1558b0]"
            >
              Search
            </button>
          </div>

          <div className="mt-3 flex justify-center gap-6 text-sm text-[#5f6368]">
            <label className="flex items-center gap-1">
              <input
                type="radio"
                name="mode"
                value="plain"
                checked={mode === "plain"}
                onChange={() => setMode("plain")}
              />
              Plain
            </label>
            <label className="flex items-center gap-1">
              <input
                type="radio"
                name="mode"
                value="regex"
                checked={mode === "regex"}
                onChange={() => setMode("regex")}
              />
              Regex
            </label>
          </div>
        </form>

        {error && (
          <div
            role="alert"
            className="mt-5 text-[0.95rem] text-[#d93025]"
          >
            {error}
          </div>
        )}

        {loading && <p className="mt-5 text-[#5f6368]">Searching history…</p>}
      </div>

      {hasSubmitted && !loading && (
        <section
          className="mx-auto mb-16 mt-10 w-full max-w-[900px] px-6"
        >
          {results.length === 0 && !error ? (
            <p className="text-[#5f6368]">
              No results yet. Try a broader query or switch modes.
            </p>
          ) : (
            results.map((hit) => (
              <div
                key={`${hit.file_path}-${hit.first_match.commit_sha}`}
                className="mb-10"
              >
                <h2
                  className="mb-1 text-[1.2rem] text-[#1a0dab] break-all"
                >
                  {hit.file_path}
                </h2>
                <p className="mb-2 text-[#5f6368]">
                  Matching {hit.first_match.lines.length} line
                  {hit.first_match.lines.length === 1 ? "" : "s"}
                </p>

                {renderMatchSection("First seen", hit.first_match)}
                {renderMatchSection("Last seen", hit.last_match)}
              </div>
            ))
          )}
        </section>
      )}
    </div>
  );
}

export default App;
