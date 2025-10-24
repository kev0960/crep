import type { CSSProperties, FormEvent, ReactNode } from "react";
import { useState } from "react";
import { executeSearch } from "./api/client";
import type { LineMatch, MatchDetail, SearchHit, SearchMode } from "./api/types";

const CREP_ASCII = [
  "  _____ ____  ______ _____ ",
  " / ____/ __ \\|  ____|  __ \\",
  "| |   | |  | | |__  | |__) |",
  "| |   | |  | |  __| |  _  / ",
  "| |___| |__| | |____| | \\ \\ ",
  " \\_____|\\___/|______|_|  \\_\\",
].join("\n");

const pageStyles: CSSProperties = {
  minHeight: "100vh",
  background: "#f8f9fa",
  color: "#202124",
  fontFamily: "system-ui, -apple-system, BlinkMacSystemFont, sans-serif",
};

const centeredColumn: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  padding: "0 1.5rem",
};

const searchBoxStyles: CSSProperties = {
  width: "min(600px, 100%)",
  display: "flex",
  gap: "0.75rem",
  alignItems: "center",
  background: "#fff",
  borderRadius: "999px",
  padding: "0.75rem 1.25rem",
  boxShadow:
    "0 1px 4px rgba(0, 0, 0, 0.06), 0 2px 8px rgba(0, 0, 0, 0.04)",
};

function highlightLine(line: LineMatch) {
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

    const highlighted =
      line.content.slice(start, end) || highlight.term;

    segments.push(
      <mark key={`mark-${line.line_number}-${index}`}>
        {highlighted}
      </mark>,
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
}

function renderMatchSection(label: string, detail: MatchDetail | null) {
  if (!detail) {
    return null;
  }

  const date = new Date(detail.commit_date);
  const formattedDate = Number.isNaN(date.getTime())
    ? detail.commit_date
    : date.toLocaleString();

  return (
    <article
      style={{
        borderRadius: "12px",
        border: "1px solid rgba(0,0,0,0.08)",
        padding: "1rem 1.25rem",
        background: "#fff",
        boxShadow: "0 1px 2px rgba(0,0,0,0.04)",
        marginTop: "1rem",
      }}
    >
      <header style={{ marginBottom: "0.5rem" }}>
        <strong>{label}</strong>
        <div
          style={{
            display: "flex",
            flexWrap: "wrap",
            gap: "0.75rem",
            fontSize: "0.9rem",
            color: "#5f6368",
            marginTop: "0.25rem",
          }}
        >
          <span>
            Commit:{" "}
            <code>
              {detail.commit_sha.substring(0, 7)}
            </code>
          </span>
          <span>When: {formattedDate}</span>
        </div>
        {detail.commit_summary && (
          <p style={{ marginTop: "0.5rem", color: "#3c4043" }}>
            {detail.commit_summary}
          </p>
        )}
      </header>

      <div
        style={{
          background: "#f1f3f4",
          borderRadius: "8px",
          padding: "0.75rem",
          overflowX: "auto",
          fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
          fontSize: "0.95rem",
        }}
      >
        {detail.lines.map((line) => (
          <div
            key={`${detail.commit_sha}-${line.line_number}`}
            style={{
              display: "flex",
              gap: "1rem",
              alignItems: "flex-start",
            }}
          >
            <span
              style={{
                color: "#9aa0a6",
                minWidth: "3rem",
                textAlign: "right",
              }}
            >
              {line.line_number}
            </span>
            <code style={{ whiteSpace: "pre-wrap" }}>
              {highlightLine(line)}
            </code>
          </div>
        ))}
      </div>
    </article>
  );
}

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
      setError(
        err instanceof Error ? err.message : "Unknown search error.",
      );
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={pageStyles}>
      <div
        style={{
          ...centeredColumn,
          paddingTop: hasSubmitted ? "3rem" : "18vh",
          transition: "padding 0.3s ease",
        }}
      >
        <pre
          style={{
            fontFamily:
              "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
            textAlign: "center",
            fontSize: hasSubmitted ? "1rem" : "1.3rem",
            marginBottom: "2rem",
            letterSpacing: "0.05rem",
            color: "#202124",
          }}
        >
          {CREP_ASCII}
        </pre>

        <form onSubmit={handleSubmit} style={{ width: "100%" }}>
          <div style={searchBoxStyles}>
            <input
              type="text"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search git history…"
              style={{
                flex: 1,
                border: "none",
                outline: "none",
                fontSize: "1.05rem",
                background: "none",
              }}
              aria-label="Search query"
            />
            <button
              type="submit"
              style={{
                padding: "0.55rem 1.2rem",
                borderRadius: "999px",
                border: "none",
                background: "#1a73e8",
                color: "#fff",
                fontWeight: 600,
                cursor: "pointer",
                fontSize: "0.95rem",
              }}
            >
              Search
            </button>
          </div>

          <div
            style={{
              display: "flex",
              justifyContent: "center",
              gap: "1.5rem",
              marginTop: "0.75rem",
              color: "#5f6368",
              fontSize: "0.9rem",
            }}
          >
            <label style={{ display: "flex", alignItems: "center", gap: 4 }}>
              <input
                type="radio"
                name="mode"
                value="plain"
                checked={mode === "plain"}
                onChange={() => setMode("plain")}
              />
              Plain
            </label>
            <label style={{ display: "flex", alignItems: "center", gap: 4 }}>
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
            style={{
              marginTop: "1.25rem",
              color: "#d93025",
              fontSize: "0.95rem",
            }}
          >
            {error}
          </div>
        )}

        {loading && (
          <p style={{ marginTop: "1.25rem", color: "#5f6368" }}>
            Searching history…
          </p>
        )}
      </div>

      {hasSubmitted && !loading && (
        <section
          style={{
            width: "min(900px, 100%)",
            margin: "2.5rem auto 4rem",
            padding: "0 1.5rem",
          }}
        >
          {results.length === 0 && !error ? (
            <p style={{ color: "#5f6368" }}>
              No results yet. Try a broader query or switch modes.
            </p>
          ) : (
            results.map((hit) => (
              <div
                key={`${hit.file_path}-${hit.first_match.commit_sha}`}
                style={{ marginBottom: "2.5rem" }}
              >
                <h2
                  style={{
                    fontSize: "1.2rem",
                    color: "#1a0dab",
                    marginBottom: "0.25rem",
                    wordBreak: "break-all",
                  }}
                >
                  {hit.file_path}
                </h2>
                <p style={{ color: "#5f6368", marginBottom: "0.5rem" }}>
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
