# One-off: fetch the Seveso river course from OpenStreetMap (Overpass) and write it as a
# GeoJSON asset for the frontend map. Offline tooling only (CLAUDE.md: Python never on the
# runtime path) — the output is committed, the app never calls Overpass.
#
#   python3 scripts/explore/seveso_river_geojson.py
#
# Data © OpenStreetMap contributors, ODbL — attribution shown on the map.

import json
import urllib.parse
import urllib.request
from pathlib import Path

OUT = (
    Path(__file__).resolve().parents[2]
    / "frontend"
    / "src"
    / "assets"
    / "seveso-river.json"
)
# Brianza/Como springs down to Milan, generously bounded.
QUERY = """
[out:json][timeout:90];
way["waterway"="river"]["name"="Seveso"](45.4,8.9,45.9,9.4);
out geom;
"""


def main() -> None:
    body = urllib.parse.urlencode({"data": QUERY}).encode()
    req = urllib.request.Request(
        "https://overpass-api.de/api/interpreter",
        data=body,
        headers={"User-Agent": "argine-geo-prep/0.1"},
    )
    with urllib.request.urlopen(req, timeout=120) as resp:
        data = json.load(resp)

    # One MultiLineString of every river way, coordinates rounded to ~1 m (5 decimals).
    lines = [
        [[round(p["lon"], 5), round(p["lat"], 5)] for p in way["geometry"]]
        for way in data["elements"]
        if way["type"] == "way" and "geometry" in way
    ]
    geojson = {
        "type": "Feature",
        "properties": {
            "name": "Seveso",
            "attribution": "© OpenStreetMap contributors (ODbL)",
        },
        "geometry": {"type": "MultiLineString", "coordinates": lines},
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(geojson, separators=(",", ":")) + "\n")
    n_points = sum(len(line) for line in lines)
    print(
        f"wrote {OUT} — {len(lines)} segments, {n_points} points, {OUT.stat().st_size} bytes"
    )


if __name__ == "__main__":
    main()
