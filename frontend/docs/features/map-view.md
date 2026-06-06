# Feature: map view

## Overview

A **MapLibre GL** map of the Seveso above the station selector: the river course as a blue
line, the official **PGRA flood-hazard areas** (toggleable, with legend) and one marker per
station. Clicking a marker selects that station for the charts below — the map and the
`<select>` drive the same state. First steps of the GIS roadmap (IDEAS.md §6-bis: the
dynamic HAND flood extent comes later).

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
- **PGRA hazard areas:** a static GeoJSON asset (`src/assets/seveso-pgra.json`, ~133 KB)
  with the **Direttiva Alluvioni** flood-hazard polygons for the Seveso, extracted once from
  the Regione Lombardia geoportale (`scripts/explore/seveso_pgra_geojson.py` — the ArcGIS
  `query` op withholds geometry on that server; `identify` with an envelope returns it).
  Three scenarios as separate fill layers: **P3** frequent (red), **P2** infrequent
  (orange), **P1** rare (yellow), 35% opacity under the river line. A legend checkbox
  toggles all three via `setLayoutProperty`. Data © Regione Lombardia, **IODL 2.0** —
  attribution on the map. These are the official _static_ hazard zones; the _dynamic_
  forecast-driven extent (HAND) is Phase 3 and will overlay them.
- **Coloring by alert status** (level vs thresholds) is deferred: no threshold data is
  seeded yet — when it lands, the marker color becomes a function of latest level/forecast.

## Files / code

- `src/components/MapView.tsx` — map init (once), PGRA + river layers on load, hazard
  toggle state, markers rebuilt on `stations`/`selectedId` change;
  `MapViewProps { stations, selectedId, onSelect }`.
- `src/App.tsx` — renders `<MapView>` wired to the existing selection state.
- `src/assets/seveso-river.json` / `seveso-pgra.json` — committed geometry assets
  (prettier-ignored, minified).
- `scripts/explore/seveso_river_geojson.py` / `seveso_pgra_geojson.py` — offline
  regeneration of the assets.
- `src/test/maplibre-mock.ts` — fake maplibre for jsdom (no WebGL); records maps/markers,
  can fire `load`.
- `src/components/MapView.test.tsx` — markers per station, selection highlight, click →
  `onSelect`, a11y attributes, layers added on load, legend toggle ↔ layer visibility.
  `e2e/chart.spec.ts` asserts map + marker + legend in a real browser with **OSM tile
  requests stubbed** (no external calls in CI).

## Contract

- Props in, callback out — no fetching inside the component.
- External runtime dependency: OSM raster tiles (browser only; stubbed in E2E).
- New dependency: `maplibre-gl` (BSD-3-Clause).

## Status

**Implemented** (stations + river line + PGRA hazard areas with legend/toggle). The dynamic
HAND flood extent is planned — IDEAS.md §6-bis / Phase 3.

## Open questions

- Tile provider for production (OSM tile policy is for light use; MapTiler needs a key —
  which must ship via env, not the bundle).
- Marker color from alert status once thresholds are seeded.
- PGRA covers the _main network_ (RP) layers only; the secondary-network (RSP) polygons
  around Milan could be added the same way if useful.
- Popup with scenario details on hazard-area click; PAI fasce fluviali (A/B/C) as an
  additional layer.
- Cluster/expand if station count grows beyond the Seveso three.
