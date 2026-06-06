# One-off: fetch the PGRA flood-hazard areas (Direttiva Alluvioni 2007/60/CE) for the
# Seveso from the Regione Lombardia geoportale and write them as a GeoJSON asset for the
# frontend map. Offline tooling only (CLAUDE.md: Python never on the runtime path) — the
# output is committed, the app never calls the geoportale.
#
#   python3 scripts/explore/seveso_pgra_geojson.py
#
# Source: ArcGIS service wms/direttiva_alluvioni_2015_wms (the REST `query` operation
# withholds geometry on this server; `identify` with an envelope returns it).
# Scenarios: H = frequent (P3), M = infrequent (P2), L = rare (P1).
# Data: Regione Lombardia, IODL 2.0 — attribution shown on the map.

import json
import urllib.parse
import urllib.request
from pathlib import Path

OUT = (
    Path(__file__).resolve().parents[2]
    / "frontend"
    / "src"
    / "assets"
    / "seveso-pgra.json"
)
BASE = (
    "https://www.cartografia.servizirl.it/arcgis/rest/services"
    "/wms/direttiva_alluvioni_2015_wms/MapServer/identify"
)
# (layer id, PGRA scenario) for the "Pericolosità RP" (main network) layers.
LAYERS = [(3, "P3"), (7, "P2"), (11, "P1")]
# Seveso corridor, generously bounded (same as the river asset).
BBOX = {"xmin": 8.98, "ymin": 45.44, "xmax": 9.25, "ymax": 45.87}


def fetch_layer(layer_id: int) -> list[dict]:
    params = {
        "geometry": json.dumps({**BBOX, "spatialReference": {"wkid": 4326}}),
        "geometryType": "esriGeometryEnvelope",
        "sr": "4326",
        "layers": f"all:{layer_id}",
        "tolerance": "0",
        "mapExtent": f"{BBOX['xmin']},{BBOX['ymin']},{BBOX['xmax']},{BBOX['ymax']}",
        "imageDisplay": "800,600,96",
        "returnGeometry": "true",
        "geometryPrecision": "5",
        "f": "json",
    }
    url = f"{BASE}?{urllib.parse.urlencode(params)}"
    req = urllib.request.Request(url, headers={"User-Agent": "argine-geo-prep/0.1"})
    with urllib.request.urlopen(req, timeout=120) as resp:
        return json.load(resp)["results"]


def signed_area(ring: list[list[float]]) -> float:
    return sum(
        (ring[i][0] * ring[(i + 1) % len(ring)][1])
        - (ring[(i + 1) % len(ring)][0] * ring[i][1])
        for i in range(len(ring))
    )


def point_in_ring(pt: list[float], ring: list[list[float]]) -> bool:
    x, y = pt
    inside = False
    for i in range(len(ring)):
        x1, y1 = ring[i]
        x2, y2 = ring[(i + 1) % len(ring)]
        if (y1 > y) != (y2 > y) and x < (x2 - x1) * (y - y1) / (y2 - y1) + x1:
            inside = not inside
    return inside


def esri_rings_to_multipolygon(rings: list[list[list[float]]]) -> list:
    """Esri polygons list rings flat: exterior rings clockwise (negative shoelace area),
    holes counterclockwise. GeoJSON needs holes nested under their exterior — assign each
    hole to the first exterior that contains its first vertex."""
    outers = [r for r in rings if signed_area(r) < 0]
    holes = [r for r in rings if signed_area(r) >= 0]
    polygons = [[o] for o in outers]
    for hole in holes:
        for poly in polygons:
            if point_in_ring(hole[0], poly[0]):
                poly.append(hole)
                break
    return polygons


def main() -> None:
    features = []
    for layer_id, scenario in LAYERS:
        results = [
            r
            for r in fetch_layer(layer_id)
            if r["attributes"].get("Denominazione elemento idrico") == "Seveso"
        ]
        for r in results:
            features.append(
                {
                    "type": "Feature",
                    "properties": {
                        "scenario": scenario,
                        "tritorno_anni": int(
                            r["attributes"].get("Tempo di ritorno", 0)
                        ),
                    },
                    "geometry": {
                        "type": "MultiPolygon",
                        "coordinates": esri_rings_to_multipolygon(
                            r["geometry"]["rings"]
                        ),
                    },
                }
            )
    collection = {
        "type": "FeatureCollection",
        "properties": {
            "name": "PGRA Seveso",
            "attribution": "Dati PGRA © Regione Lombardia (IODL 2.0)",
        },
        "features": features,
    }
    OUT.write_text(json.dumps(collection, separators=(",", ":")) + "\n")
    print(f"wrote {OUT} — {len(features)} features, {OUT.stat().st_size} bytes")


if __name__ == "__main__":
    main()
