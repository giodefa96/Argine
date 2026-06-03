# Feature: Level chart (Seveso)

## Overview

A time-series chart of the Seveso river **level** for a selected station, with **rainfall**
overlaid on a second axis so the two correlate visually (rain precedes the level rise). Fed by
the backend read API; a station selector switches between the three Seveso gauges.

## Design

- **Data layer:** a small typed client (`src/lib/api.ts`) over the backend read API, wrapped
  in **TanStack Query** hooks (`src/hooks/queries.ts`). Base URL from `VITE_API_URL`
  (default `http://localhost:8080`). The query for observations is `enabled` only once a
  station id is known.
- **Chart:** **uPlot** (fast vanilla-JS canvas lib) wrapped in `LevelChart`. uPlot is driven
  imperatively in a `useEffect` and rebuilt when the data changes; timestamps → unix seconds on
  the x axis (`scales.x.time`). **Two y axes:** level as a line on the left (`m`), rain as bars on
  the right (`mm`). Series are aligned on the union of their timestamps (gaps → `null`).
- **States:** `StationLevel` fetches both metrics (`level_m`, `rain_mm`) for the station and
  handles loading / error (`role="alert"`) / empty / data on the level query, showing the latest
  level above the chart. Rain is optional — the chart degrades to level-only when absent.
- **App shell:** `App` lists stations in a `<select>` (labelled, with `id`/`name` for a11y),
  defaults to the first, and renders `StationLevel` for the selection.

## Files / code

- `src/lib/api.ts` — `fetchStations`, `fetchObservations`, `Station`/`Observation` types.
- `src/hooks/queries.ts` — `useStations`, `useObservations`.
- `src/components/LevelChart.tsx` — uPlot wrapper.
- `src/components/StationLevel.tsx` — per-station loader + states.
- `src/App.tsx` — station selector + composition; `src/main.tsx` — `QueryClientProvider`.

## Contract

- Reads `GET /stations` and, per station, `GET /stations/{id}/observations?metric=level_m` and
  `?metric=rain_mm` (`limit=2000`, defaults to the backend's last-30-days window). Requires
  backend CORS to allow the frontend origin.
- Env: `VITE_API_URL` (build/runtime base URL of the backend).

## Testing

- **Vitest + MSW** (`src/lib/api.test.ts`, `src/components/StationLevel.test.tsx`,
  `src/App.test.tsx`): client params + error, the loading/error/empty/data states, and that
  the station list populates. uPlot is mocked (no canvas in jsdom).
- **Playwright** (`e2e/chart.spec.ts`): with the API mocked via `page.route`, the chart's
  canvas renders and the latest value shows. Verified manually against the real backend with
  ingested Seveso data.

## Status

**Implemented** (observed level + observed rainfall overlay). Forecast overlay and the stations
map come later.

## Open questions

- Time-range selector / slider (the backend already supports `from`/`to`); add when needed.
- Downsampling for long ranges (ties into backend continuous aggregates).
- Styling is minimal (plain CSS); Tailwind + layout polish is a later pass.
