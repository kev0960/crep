# crep-server – Current State & Integration Plan

`crep-server` is an Axum + Leptos application scaffold ready to surface the Git history search UI over HTTP.

## Runtime (`src/main.rs`)
- Loads configuration via `get_configuration`, extracts `LeptosOptions`, and generates the Leptos route list (`generate_route_list(App)`).
- Builds an Axum `Router` with `leptos_routes` for SSR + hydration, a static file/error fallback, and stores `LeptosOptions` in application state.
- Binds a TCP listener on `site_addr`, logs the bound URL, and serves the app with `axum::serve` on Tokio.

## UI Shell (`src/app.rs`)
- `shell` renders the base HTML document, wiring hydration and autoreload scripts provided by `leptos`.
- `App` sets up meta context, attaches the stylesheet, and wraps routes in a `Router` + `Routes` pair (single `HomePage` today).
- `HomePage` is the template counter from the Leptos starter, demonstrating reactive state via `RwSignal` and event handlers.

## Bringing Search Online
1. Load a `GitIndex` during startup (e.g., in `main` before constructing the router) and store it in shared state alongside a `Repository` path.
2. Add Axum handlers that accept queries, construct a `GitSearcher`, and respond with JSON results (`SearchResult` or custom DTOs).
3. Replace `HomePage` with Leptos components that call the new endpoints (or leverage `wasm-bindgen` to share client logic) and render highlighted line snippets.
4. Consider streaming or pagination for large result sets; reuse `crep_indexer::search::result_viewer` utilities server-side to avoid reimplementing highlighting.

The server currently serves as a starting point—wire in the shared crate APIs above to deliver real search functionality.
