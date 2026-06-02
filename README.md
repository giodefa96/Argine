# Argine

River level monitoring and short-term forecasting from meteorological models, with risk
alerts. **First target river: the Seveso** (Brianza/Como basin → Milan).

> ⚠️ Early design phase — no application code yet. See the documents below.

## Documents
- [`IDEAS.md`](./IDEAS.md) — design: architecture, stack, forecast model (point + spatial), deploy, roadmap.
- [`SECURITY.md`](./SECURITY.md) — security baseline, CI pipeline, supply-chain defense, checklist.
- [`CLAUDE.md`](./CLAUDE.md) — guidance for AI agents working in this repo.

## Stack (planned)
- **Backend:** Rust (Axum, Tokio, sqlx + PostgreSQL/TimescaleDB), ONNX inference.
- **Frontend:** React + TypeScript (Vite), MapLibre GL — PWA.
- **Offline:** Python for ML training and geo/HAND preprocessing.
- **Deploy:** Docker Compose.

## Data sources
ARPA Lombardia (hydrometry) · Open-Meteo (rain forecast) · PGRA/PAI + DTM (flood mapping).

## Development
```sh
make hooks     # install local git hooks (secret scan + pre-push checks)
make security  # run the full local security gate (mirrors CI)
```
