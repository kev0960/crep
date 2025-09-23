# crep-server – Current State & Integration Plan

`crep-server` is an Axum + Leptos application scaffold meant to host a web UI for the Git history search engine.

## Runtime (`src/main.rs`)
- Loads configuration via `crep_server::app::get_configuration` (provided by the Leptos template).
- Builds an Axum `Router` that serves server-rendered Leptos routes and static assets (fallback handler).
- Listens on `LeptosOptions.site_addr` and runs with `tokio`.

## UI Shell (`src/app.rs`)
- Defines `shell` which wires Leptos hydration scripts, auto-reload, and mounts `<App/>`.
- `<App/>` sets up meta context, stylesheet, and router; today it serves a single `HomePage` route with a click counter.

## Next Steps to Integrate Search
1. Load a `GitIndex` (built via CLI or on startup) during server initialization and store it in application state.
2. Expose search endpoints (e.g., Axum handlers returning JSON) that construct `GitSearcher` instances per request.
3. Replace the placeholder `HomePage` with UI components that call those endpoints and render highlighted results.
4. Consider streaming responses for large result sets; reuse `GitSearchResultViewer` logic or port highlighting into the frontend.

Until those steps are implemented, this crate remains a template but is ready to host the shared library once routing and state management are in place.
