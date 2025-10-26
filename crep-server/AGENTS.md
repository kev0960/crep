# crep-server Notes

The `crep-server` crate now targets a JSON-first Axum backend with a React front end.

## Server (`server`)

- `main.rs` bootstraps tracing, binds to `BIND_ADDR` (defaults to `127.0.0.1:3000`), and serves the router.
- `lib.rs` exposes `router()` which aggregates routes; add per-module routers here as the API grows.
- `api.rs` is the initial REST surface (currently only `/api/health`). Expand this into submodules (`search`, `indices`, etc.) as functionality is added.

### OpenAPI

- Add `utoipa` + `utoipa-swagger-ui` when you are ready to generate specs.
- Derive `ToSchema` on DTOs in `api.rs` or dedicated `dto` modules.
- Surface docs at `/docs` and `/docs.json`, then generate TypeScript bindings for the SPA.

## Web (`web`)

- Vite + React + TypeScript, proxied to the Axum server during development.
- Use PNPM workspaces (`pnpm-workspace.yaml`) to share future TS packages (e.g., generated API client).
- SPA bootstraps from `src/main.tsx` and calls `/api/health` as a smoke test.

## Next Integration Steps

1. Load a `GitIndex` in server state and expose search routes.
2. Formalize API contracts with OpenAPI and generate a shared TS client.
3. Serve `web/dist` from Axum for production deploys (e.g., mount with `tower_http::services::ServeDir`).
