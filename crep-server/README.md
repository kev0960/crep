# crep-server

Axum HTTP server paired with a React (Vite + PNPM) single-page app. This crate will expose the Git history search APIs that power the broader `crep` workspace.

## Layout
- `server`: Rust application serving JSON APIs and (in production) the built SPA assets.
- `web`: React + TypeScript SPA managed by PNPM.
- `pnpm-workspace.yaml`: declares the workspace so both apps share tooling.

## Prerequisites
- Rust toolchain (edition 2021).
- `pnpm` (`npm install -g pnpm`).

## Development
1. Install JS dependencies:
   ```bash
   cd web
   pnpm install
   ```
2. Start the Axum server (requires a persisted `GitIndex` and the repo it was produced from):
   ```bash
   export CREP_INDEX_PATH=/path/to/index.bin
   export CREP_REPO_PATH=/path/to/indexed/repo
   cargo run --manifest-path server/Cargo.toml
   ```
   The server listens on `127.0.0.1:3000` by default. Override with `BIND_ADDR`.
3. In another terminal, run the SPA dev server:
   ```bash
   pnpm --dir web dev
   ```
   Vite proxies `/api/*` to the Axum server.

## Search API
- `POST /api/search` accepts `{ query, mode?, limit? }` and returns the first/last commits that contained the match alongside highlighted context.
- OpenAPI is served from `/docs.json` and a matching TypeScript definition bundle from `/docs.ts`.
- The SPA consumes those contracts via `web/src/api/types.ts` and `web/src/api/client.ts`.

## Production Build
1. Bundle the SPA:
   ```bash
   pnpm --dir web build
   ```
   Output goes to `web/dist`.
2. Serve the built assets by teaching Axum to mount the `dist` directory (TBD implementation).

## OpenAPI & TypeScript
- DTOs live in `server/src/api/search.rs` and derive `utoipa::ToSchema`.
- Generate the specification without running the server:
  ```bash
  cargo run --manifest-path server/Cargo.toml --bin openapi-export -- --format json --out openapi.json
  cargo run --manifest-path server/Cargo.toml --bin openapi-export -- --format ts --out web/src/api/types.ts
  ```
- The React app checks in the TypeScript definitions at `web/src/api/types.ts`. Run `pnpm --dir web generate:api` to refresh them.
- When the server is running, `/docs.json` and `/docs.ts` still expose the same contracts over HTTP.
