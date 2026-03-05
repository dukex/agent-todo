# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`agent-todo` is a Rust-based HTTP API running on **Cloudflare Workers** using the `worker` crate with `axum` as the routing framework. The compiled output is a WebAssembly (WASM) `cdylib` deployed via Wrangler. Storage uses Cloudflare D1 (SQLite) and KV.

## Commands

### Build
```bash
cargo install -q "worker-build@^0.7" && worker-build --release
```

### Local development
```bash
npx wrangler dev
```

### Deploy
```bash
npx wrangler deploy
```

### Check / lint
```bash
cargo check
cargo clippy
```

## Architecture

- `src/lib.rs` — entry point. The `#[event(fetch)]` macro registers the Cloudflare Workers fetch handler. Routing is done via `axum::Router` and dispatched through `tower_service::Service::call`.
- `wrangler.toml` — Cloudflare Workers configuration: D1 database binding (`DB`), KV namespace binding (`KV`), and build command.
- WASM target is built by `worker-build`; the output goes to `build/index.js`.

### Key bindings (wrangler.toml)
| Binding | Type | Purpose |
|---------|------|---------|
| `DB` | D1 | SQLite database for todo persistence |
| `KV` | KV namespace | Key-value storage |

The `Env` object passed to the fetch handler is how bindings are accessed at runtime.
