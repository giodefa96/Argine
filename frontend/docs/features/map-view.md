# Feature: map view

## Overview

A **MapLibre GL** map of the Seveso above the station selector: the river course as a blue
line plus one marker per station. Clicking a marker selects that station for the charts
below — the map and the `<select>` drive the same state. First step of the GIS roadmap
(IDEAS.md §6-bis: PGRA/PAI risk layers and dynamic HAND flood extent come later).

## Design

- **Base map:** free **OSM raster tiles** (`tile.openstreetmap.org`) with the required
  attribution — no API key, fine for development/low traffic. Swap for a vector-tile
  provider (MapTiler/Protomaps) before any real launch (usage policy + nicer styling).
- **River course:** a static **GeoJSON asset** (`src/assets/seveso-river.json`, ~47 KB,
  MultiLineString) extracted **once** from OpenStreetMap via Overpass with
  `scripts/explore/seveso_river_geojson.py` and committed — the app never calls Overpass
  (the river doesn't move). Data © OpenStreetMap contributors (ODbL), attribution on the map.
- **Markers:** built from the `stations` prop (`lat`/`lon` from `GET /stations` — the data
  layer stays in `App`); selected station red, others blue; popup with the station name;
  marker elements get `role="button"` + `aria-label` for accessibility and tests.
- **Coloring by alert status** (level vs thresholds) is deferred: no threshold data is
  seeded yet — when it lands, the marker color becomes a function of latest level/forecast.

## Files / code

- `src/components/MapView.tsx` — map init (once), river source/layer on load, markers
  rebuilt on `stations`/`selectedId` change; `MapViewProps { stations, selectedId, onSelect }`.
- `src/App.tsx` — renders `<MapView>` wired to the existing selection state.
- `src/assets/seveso-river.json` — committed river geometry (prettier-ignored, minified).
- `scripts/explore/seveso_river_geojson.py` — offline regeneration of the asset.
- `src/test/maplibre-mock.ts` — fake maplibre for jsdom (no WebGL); records markers.
- `src/components/MapView.test.tsx` — markers per station, selection highlight, click →
  `onSelect`, a11y attributes. `e2e/chart.spec.ts` asserts map + marker in a real browser
  with **OSM tile requests stubbed** (no external calls in CI).

## Contract

- Props in, callback out — no fetching inside the component.
- External runtime dependency: OSM raster tiles (browser only; stubbed in E2E).
- New dependency: `maplibre-gl` (BSD-3-Clause).

## Status

**Implemented** (stations + river line). Risk layers (PGRA/PAI, HAND flood extent) planned —
see IDEAS.md §6-bis / Phase 3.

## Open questions

- Tile provider for production (OSM tile policy is for light use; MapTiler needs a key —
  which must ship via env, not the bundle).
- Marker color from alert status once thresholds are seeded.
- Cluster/expand if station count grows beyond the Seveso three.
