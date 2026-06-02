# Feature: Health endpoint

## Overview
A minimal liveness endpoint used by load balancers, container orchestrators, and uptime
checks to verify the process is up and serving HTTP.

## Design
Stateless handler returning a fixed JSON body. No auth (public read), no dependencies — it
must succeed even if downstream systems (DB, external APIs) are down, so it reflects
*process* liveness, not full readiness.

## Files / code
- `src/main.rs`
  - route: `GET /health` → `health`
  - `health()` — returns `Json(json!({ "status": "ok" }))`

## Contract
- **Request:** `GET /health`
- **Response:** `200 OK`, body `{"status":"ok"}`, content-type `application/json`

## Status
✅ Implemented.

## Open questions
- Add a separate `/ready` (readiness) endpoint that checks DB connectivity once persistence
  lands? Liveness vs readiness should stay distinct.
