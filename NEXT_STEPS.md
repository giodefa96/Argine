# Next steps

Ordered, dependency-aware backlog. Each item is a feature delivered through the established
flow: `git flow feature start <name>` → code **+** tests **+** `docs/features/<name>.md` and
`ARCHITECTURE.md` updated → PR into `develop` → all gates green. See [`CLAUDE.md`](./CLAUDE.md)
(Definition of Done) and [`IDEAS.md`](./IDEAS.md) (product vision) for context.

## Foundation — done ✅
- Design docs, security baseline, supply-chain defense.
- CI: security gate, dependency-review, CodeQL, staged tests, agent reviews (Copilot + Claude).
- git-flow branching (`main`/`develop`), protected branches.
- Scaffold: Rust/Axum backend (`/health`, fail-fast config), React/Vite/pnpm frontend.
- Testing + lint + pre-commit hooks (installed and verified).

---

## MVP — in priority order

### 1. Infra: PostgreSQL + TimescaleDB + Docker Compose ✅ (PR #8 → `develop`)
- **Goal:** local dev stack and persistence layer.
- **Scope:** `docker-compose.yml` (db = `timescale/timescaledb`, backend); `sqlx` wired with
  `Cargo.lock`; first migration; `DATABASE_URL` config (fail-fast); `/ready` endpoint that
  checks DB connectivity (distinct from `/health` liveness).
- **Tests:** `sqlx::test` harness against an ephemeral DB; `/ready` integration test.
- **Docs:** `backend/docs/features/persistence.md`.

### 2. Domain model + repositories ✅ (PR #9 → `develop`)
- **Goal:** persist the core entities.
- **Scope:** migrations + types for `station`, `observation`, `threshold`; a thin repository
  layer with parameterized queries only.
- **Tests:** repository CRUD via `sqlx::test`; threshold mapping.
- **Docs:** `backend/docs/features/domain-model.md` + schema in `ARCHITECTURE.md`.

### 3. ARPA hydrometry ingestion (Seveso) ✅ (PR #11; observed rainfall in PR #14)
- **Goal:** pull near-real-time level for the Seveso station.
- **Scope:** scheduled job (`tokio-cron-scheduler`); ARPA Lombardia client (`reqwest`);
  normalize → `observation`. Pick the MVP station (open question in IDEAS.md §12).
- **Tests:** parser/normalizer unit tests; HTTP mocked with **`wiremock`** (no real API in CI).
- **Docs:** `backend/docs/features/arpa-ingestion.md`.

### 4. Open-Meteo rain-forecast ingestion ✅
- **Goal:** forecast precipitation over the basin.
- **Scope:** Open-Meteo client; store `weather_forecast`; basin defined as a point/bbox first.
- **Tests:** client + normalizer with `wiremock` fixtures.
- **Docs:** `backend/docs/features/weather-ingestion.md`.

### 5. Read API ✅ (PR #12 → `develop`)
- **Goal:** expose data to the frontend.
- **Scope:** `GET /stations`, `GET /stations/{id}`, `GET /stations/{id}/observations`
  (`?from&to`, bounded pagination, input validation).
- **Tests:** integration tests via `oneshot` against a seeded test DB.
- **Docs:** update API surface in `backend/docs/ARCHITECTURE.md`.

### 6. Frontend: data layer + level chart ✅ (PR #13; rain/level correlation in PR #14)
- **Goal:** first real view — the Seveso level over time.
- **Scope:** **TanStack Query** client; **uPlot** time-series chart; basic layout.
- **Tests:** Vitest + Testing Library with **MSW**-mocked API; one Playwright journey
  ("chart renders with data").
- **Docs:** `frontend/docs/features/level-chart.md`.

---

## Phase 1 — forecast & alerts (after MVP)
- **Baseline forecast** ✅: lag-based model in Rust + `GET /stations/{id}/forecast`
  (`proptest` invariants: more rain ⇒ never-lower predicted level). α/β still uncalibrated.
- **Alert engine:** compare forecast vs thresholds → `alert`; SSE stream + **Telegram** bot.
- **PWA + Web Push:** installable frontend, push notifications for alerts.

## Phase 2 — ML
- Python training pipeline (XGBoost/LightGBM) → **ONNX** export → inference in Rust (`ort`/`tract`).
- Backtesting on historical flood events.

## Phase 3 — spatial forecasting
- DTM/LIDAR acquisition; offline **HAND** pipeline (Python) → flood-extent polygons per level.
- MapLibre dynamic risk layer + time slider; official PGRA/PAI layers.

---

## Cross-cutting activation items (not blocking, but pending)
- [ ] Add `CODECOV_TOKEN` to arm the **no-drop coverage ratchet** (currently informational).
- [ ] Once `test.yml` jobs are stable, add `backend-test` / `frontend-test` / `e2e` as
      **required checks** in branch protection (alongside `secret-scan`, `dependency-review`).
- [ ] Adopt **`cargo-vet`** to gate unvetted crate versions (SECURITY.md §13).
- [ ] Add a **devcontainer / sandboxed build** with no secrets + restricted egress (SECURITY.md §13).
- [ ] Resolve open data questions in IDEAS.md §12 (Seveso station, basin extent, DTM coverage, data licensing).
