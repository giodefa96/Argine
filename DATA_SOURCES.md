# Data sources — Seveso (feasibility & access)

Findings from probing the live ARPA Lombardia / Regione Lombardia APIs on **2026-06-03**.
Reproducible via [`scripts/explore/arpa_seveso.py`](./scripts/explore/arpa_seveso.py).
This document answers the open questions in [`IDEAS.md` §12](./IDEAS.md): *which station, what's
retrievable, with what latency and licence?*

## TL;DR
- **Data is retrievable, free, and open-source-friendly** (IODL 2.0 — attribution only). ✅
- **The 3 active hydrometric stations on the Seveso** are identified (Cantù, Paderno, Niguarda). ✅
- **History + 10-min resolution exist** (2021→today) → great for charts + ML training. ✅
- ❌ **No public real-time API for lowland river levels**: the open-data feed publishes the MI/MB/CO/LC/VA
  network with a **~18 h batch delay**. True near-real-time requires a direct arrangement with ARPA.
- The rain side is fine: **Open-Meteo** (forecast) and **ARPA radar** (geotiff) are genuinely real-time.

## The Seveso stations
Sensor id is shared across sources (`IdSensr` in SIDRO == `idsensore` in open-data):

| Sensor id | Station | Town | Elev. | Role |
|-----------|---------|------|-------|------|
| **8119** | Cantù // Asnago | Cantù (CO) | 244 m | upstream |
| **8121** | Paderno Dugnano // Palazzolo | Paderno Dugnano (MI) | 168 m | mid-basin (upstream→city lag) |
| **3118** | **Milano // Niguarda** | Milano | 139 m | city flood point → **MVP target** |

> ⚠️ `IDEAS.md` §12's guess *"Milano via Valfurva"* is wrong: Valfurva is in Valtellina (Adda).
> *"Milano v.Feltre"* is on the Lambro. Neither is the Seveso.

## The sources

### 1. ARPA open-data (Socrata) — primary, no key
Base: `https://www.dati.lombardia.it/resource/`

| Dataset | id | Content |
|---------|-----|---------|
| Station/sensor registry | `nf78-nj6b` | idsensore, tipologia, nomestazione, provincia, lat/lng, quota |
| Recent sensor values | `647i-nhxk` | 2025→now, all sensor types, 10-min |
| Hydrometric history | `3e8b-w7ay` | 2021→2025, level in cm, 10-min |

- SODA query language (`$where`, `$select`, `$group`, `$order`, `$limit`). Example — latest level for Niguarda:
  `…/647i-nhxk.json?idsensore=3118&$order=data DESC&$limit=1`
- **Latency caveat (the key finding):** only the mountain network (SO/BG) streams in ~real time.
  The **entire NW-lowland network (MI/MB/CO/LC/VA) — i.e. every Seveso station — lags ~18 h**
  (published in an early-morning daily batch). Verified: global `max(data)` = "now", but all
  Seveso/Lambro/Olona stations stuck at ~01:00 today.

### 2. SIDRO — `idro.arpalombardia.it` (best for station metadata)
A **g3w-suite v3.10.9** GIS portal (QGIS Server). Its REST API is reachable:
- Project config: `GET /api/config/2/qdjango/24`
- Vector layer data: `GET /vector/api/data/qdjango/24/{layer_id}/?formatter=1`
- Hydrometric layer display name **"Idrometri automatici attivi"** (internal id starts
  `Pluviometri_automatici_attivi_9130d654…`). It carries a **`Fiume` (river)** field — the clean way to
  enumerate stations by river, which the open-data registry lacks.
- ❌ **No time-series via API**: the `qtimeseries` plugin has an empty `layers` list; every station's
  `downlod` field points to the manual ARPA request form. SIDRO is discovery/visualisation only.

### 3. Other real-time inputs (rain side — usable now)
- **Open-Meteo** — hourly forecast precipitation by coordinates, free, no key. Real-time. (Forecast input.)
- **ARPA radar** — real-time precipitation geotiff (Desio MB, Flero BS). Open. (Nowcasting input.)
- **LIRIS / IRIS** (`iris.arpalombardia.it`) — ARPA's real-time viewer (last 14 days, *provisional* data),
  behind login. Not a programmatic feed.

## Licensing — open-source-friendly ✅
- Open-data portal: **IODL 2.0** (≈ CC-BY). Free to consult, extract, **redistribute**, and build derived
  works/apps **including commercially**. Only obligation: **attribution** — cite *"Regione Lombardia /
  ARPA Lombardia"* with a link to the licence. Compatible with this project's AGPL-3.0.
- ARPA request-form data: free, no registration; attribute *"ARPA Lombardia — Servizio Meteorologico Regionale"*.

## Getting a real-time feed (what it takes)
There is **no self-service real-time API** for lowland levels. The official request channels are batch:
- "Richiesta dati automatici" form → CSV by email within 24 h, **hourly/daily aggregates** (not live).

A real-time feed must be arranged directly with ARPA — email **`dati.idrometeo@arpalombardia.it`** with:
1. Requester identity + contact.
2. Project description: **open-source** Seveso flood early-warning, public benefit, non-exclusive.
3. Stations/sensors (8119, 8121, 3118), parameter (level), **10-min cadence**.
4. Intended use: ingest + display + short-term forecast + alerts + **public redistribution**.
5. Attribution commitment + licence link.
6. Preferred delivery (periodic pull / push / endpoint).
7. Direct questions: is there a faster endpoint for the lowland network? can the open-data publication
   lag for MI/MB be reduced? terms for real-time access?

## ⚠️ Liability / positioning
The **official** alerting system is **allertaLOM** (Regione Lombardia / Protezione Civile). ARPA real-time
data is **provisional / non-validated**. This project must be framed as **informational and complementary**
to allertaLOM — never as an authoritative alert source. State this in a disclaimer (also eases ARPA's approval).

## Recommendation
**Don't block on ARPA.** Build the full MVP pipeline now on open-data (deep history + ~18 h-delayed lowland
levels + Open-Meteo rain + real-time radar): charts, ML training, retrospective threshold-crossing analysis.
**In parallel**, email ARPA for the real-time level feed. If granted, it's a drop-in swap of the ingestion
source only (same `IdSensr`) — the architecture doesn't change.
