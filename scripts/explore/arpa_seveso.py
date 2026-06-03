#!/usr/bin/env python3
"""Exploratory probe: are ARPA Lombardia hydrometric data for the Seveso retrievable?

Throwaway data-source validation (IDEAS.md §12), NOT runtime code. The Rust backend
will do the real ingestion; this just answers "do the stations and data exist, and in
what shape?" before we commit to an ingestion design.

Sources (Socrata SODA API, no key needed for light use):
  - nf78-nj6b  station/sensor registry (idsensore, tipologia, nomestazione, lat/lng, ...)
  - 647i-nhxk  "Dati sensori meteo" — near-real-time last values
  - 3e8b-w7ay  "Livello idrometrico dal 2021" — historical hydrometric level series

Run:  python3 scripts/explore/arpa_seveso.py
"""

from __future__ import annotations

import sys
from datetime import datetime, timezone

import requests

BASE = "https://www.dati.lombardia.it/resource"
REGISTRY = f"{BASE}/nf78-nj6b.json"
REALTIME = f"{BASE}/647i-nhxk.json"
HISTORICAL = f"{BASE}/3e8b-w7ay.json"
TIMEOUT = 30

# Towns the Seveso crosses, source (Como hills) → Milan. Stations are named by
# location, not by river, so we match on these. Upper-cased for the LIKE filter.
SEVESO_TOWNS = [
    "CANTU", "CARIMATE", "CERMENATE", "LENTATE", "BARLASSINA", "SEVESO", "MEDA",
    "CESANO MADERNO", "BOVISIO", "VAREDO", "PADERNO DUGNANO", "PALAZZOLO",
    "CUSANO", "CORMANO", "BRESSO", "NIGUARDA", "MILANO", "VALFURVA",
]


def get(url: str, params: dict) -> list[dict]:
    r = requests.get(url, params=params, timeout=TIMEOUT)
    r.raise_for_status()
    return r.json()


def hydro_stations() -> list[dict]:
    """All hydrometric-level sensors in the registry."""
    return get(
        REGISTRY,
        {
            "tipologia": "Livello Idrometrico",
            "$select": "idsensore,nomestazione,provincia,quota,datastart,storico,lat,lng",
            "$limit": 500,
        },
    )


def matches_seveso(name: str) -> bool:
    up = name.upper()
    return any(town in up for town in SEVESO_TOWNS)


def latest_realtime(sensor_id: str) -> dict | None:
    rows = get(
        REALTIME,
        {"idsensore": sensor_id, "$order": "data DESC", "$limit": 1},
    )
    return rows[0] if rows else None


def historical_span(sensor_id: str) -> tuple[int, str | None, str | None]:
    """(row_count, first_ts, last_ts) in the 2021+ historical dataset."""
    count_rows = get(HISTORICAL, {"idsensore": sensor_id, "$select": "count(data)"})
    n = int(count_rows[0].get("count_data", 0)) if count_rows else 0
    if n == 0:
        return 0, None, None
    span = get(
        HISTORICAL,
        {"idsensore": sensor_id, "$select": "min(data) AS lo, max(data) AS hi"},
    )
    return n, span[0].get("lo"), span[0].get("hi")


def age(ts: str | None) -> str:
    if not ts:
        return "n/a"
    dt = datetime.fromisoformat(ts).replace(tzinfo=timezone.utc)
    delta = datetime.now(timezone.utc) - dt
    hours = delta.total_seconds() / 3600
    return f"{hours:.1f}h ago" if hours < 48 else f"{delta.days}d ago"


def main() -> int:
    print("Fetching hydrometric station registry…")
    stations = hydro_stations()
    print(f"  {len(stations)} hydrometric-level sensors statewide.\n")

    seveso = [s for s in stations if matches_seveso(s.get("nomestazione", ""))]
    if not seveso:
        print("No Seveso-basin stations matched. Widen SEVESO_TOWNS.")
        return 1

    print(f"Seveso-basin candidates ({len(seveso)}):\n")
    for s in seveso:
        sid = s["idsensore"]
        name = s.get("nomestazione", "?")
        prov = s.get("provincia", "?")
        coords = f'{s.get("lat","?")},{s.get("lng","?")}'
        start = (s.get("datastart") or "?")[:10]
        active = "active" if s.get("storico") == "N" else "historized"

        rt = latest_realtime(sid)
        if rt:
            rt_desc = f'{rt["valore"]} (cm) @ {rt["data"]} [{age(rt["data"])}] stato={rt["stato"]}'
        else:
            rt_desc = "no real-time row"

        n, lo, hi = historical_span(sid)
        hist_desc = f"{n} rows {lo[:10] if lo else '?'}→{hi[:10] if hi else '?'}" if n else "none"

        print(f"  • [{sid}] {name} ({prov}) — {active}, since {start}")
        print(f"      coords:     {coords}")
        print(f"      real-time:  {rt_desc}")
        print(f"      historical: {hist_desc}")
        print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
