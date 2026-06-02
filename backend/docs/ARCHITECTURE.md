# Backend architecture

Rust service built on **Axum** (see project-level [`IDEAS.md`](../../IDEAS.md) for the
product vision). This document tracks the *implemented* low-level structure; planned
components are marked as such.

## Startup flow (`src/main.rs`)
1. Initialize tracing (`tracing_subscriber::fmt` + `EnvFilter`, default `info`).
2. Load and validate configuration — `Config::from_env()` (`src/config.rs`). **Fails fast**
   if a secret is still the placeholder in non-local environments.
3. Build the Axum `Router`: routes + CORS layer + `TraceLayer`.
4. Bind `TcpListener` on `BIND_ADDR` and `axum::serve`.

## Modules
| Module | File | Responsibility |
|--------|------|----------------|
| entry  | `src/main.rs`   | wiring: tracing, router, server |
| config | `src/config.rs` | env-driven config + startup validation |

## HTTP surface (current)
| Method | Path      | Handler  | Description |
|--------|-----------|----------|-------------|
| GET    | `/health` | `health` | liveness probe → `{"status":"ok"}` |

## Configuration
Read from environment (see [`features/config-and-startup.md`](./features/config-and-startup.md)):

| Var | Default | Notes |
|-----|---------|-------|
| `ENVIRONMENT` | `local` | `local` \| `staging` \| `production` |
| `BIND_ADDR` | `0.0.0.0:8080` | listen address |
| `SECRET_KEY` | `changethis` | **must** be changed in non-local envs (fail-fast) |
| `CORS_ORIGINS` | _(empty)_ | comma-separated explicit allowlist |

## Runtime / deploy
- Multi-stage `Dockerfile` → **distroless, non-root** runtime image (SECURITY.md §10).
- Dependency lockfile (`Cargo.lock`) committed; built with `--locked`.

## Planned components (not yet implemented)
Tracked in [`IDEAS.md`](../../IDEAS.md); each will get a `features/` doc when built:
- Persistence: `sqlx` + PostgreSQL/**TimescaleDB**.
- Ingestion scheduler (`tokio-cron-scheduler`): ARPA hydrometry + Open-Meteo.
- Forecast engine: baseline (lag-based) → ML inference via **ONNX** (`ort`/`tract`).
- Alert engine + notification channels (SSE, Telegram, Web Push).
- Auth (JWT + Argon2) for admin/write endpoints.

## Diagram
```
env vars ──▶ Config::from_env ──(validate, fail-fast)──▶ Router
                                                          ├─ GET /health
                                                          ├─ CorsLayer (allowlist)
                                                          └─ TraceLayer
                                                              │
                                                        axum::serve(BIND_ADDR)
```
