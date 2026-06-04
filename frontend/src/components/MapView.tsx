// Map of the Seveso: river course (static GeoJSON from OSM), official PGRA flood-hazard
// areas (static GeoJSON from Regione Lombardia, toggleable) and station markers.
// Clicking a marker selects that station for the charts. Free OSM raster tiles with
// the required attribution; no API key (see frontend/docs/features/map-view.md).

import { useEffect, useRef, useState } from 'react'
import maplibregl from 'maplibre-gl'
import 'maplibre-gl/dist/maplibre-gl.css'
import type { Station } from '../lib/api'
import sevesoPgra from '../assets/seveso-pgra.json'
import sevesoRiver from '../assets/seveso-river.json'

const OSM_STYLE: maplibregl.StyleSpecification = {
  version: 8,
  sources: {
    osm: {
      type: 'raster',
      tiles: ['https://tile.openstreetmap.org/{z}/{x}/{y}.png'],
      tileSize: 256,
      attribution: '© OpenStreetMap contributors · Dati PGRA © Regione Lombardia (IODL 2.0)',
    },
  },
  layers: [{ id: 'osm', type: 'raster', source: 'osm' }],
}

// Upper Seveso basin → Milan, roughly centered between the stations.
const INITIAL_CENTER: [number, number] = [9.15, 45.62]
const INITIAL_ZOOM = 10

// PGRA scenarios, painted bottom-up so the most hazardous (P3, frequent floods) sits on
// top. Colors follow the alert palette: P1 rare = yellow … P3 frequent = red.
const PGRA_SCENARIOS = [
  { scenario: 'P1', label: 'P1 — scenario raro', color: '#eab308' },
  { scenario: 'P2', label: 'P2 — poco frequente', color: '#f97316' },
  { scenario: 'P3', label: 'P3 — frequente', color: '#dc2626' },
] as const

const pgraLayerId = (scenario: string) => `pgra-${scenario.toLowerCase()}`

export interface MapViewProps {
  stations: Station[]
  selectedId: number | null
  onSelect: (id: number) => void
}

export default function MapView({ stations, selectedId, onSelect }: MapViewProps) {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const mapRef = useRef<maplibregl.Map | null>(null)
  const markersRef = useRef<maplibregl.Marker[]>([])
  const [showHazard, setShowHazard] = useState(true)

  // The map itself: created once; river + PGRA layers added on load.
  useEffect(() => {
    if (!containerRef.current) return
    const map = new maplibregl.Map({
      container: containerRef.current,
      style: OSM_STYLE,
      center: INITIAL_CENTER,
      zoom: INITIAL_ZOOM,
    })
    map.addControl(new maplibregl.NavigationControl({ showCompass: false }))
    map.on('load', () => {
      map.addSource('pgra', { type: 'geojson', data: sevesoPgra as GeoJSON.FeatureCollection })
      for (const { scenario, color } of PGRA_SCENARIOS) {
        map.addLayer({
          id: pgraLayerId(scenario),
          type: 'fill',
          source: 'pgra',
          filter: ['==', ['get', 'scenario'], scenario],
          paint: { 'fill-color': color, 'fill-opacity': 0.35 },
        })
      }
      map.addSource('seveso', { type: 'geojson', data: sevesoRiver as GeoJSON.Feature })
      map.addLayer({
        id: 'seveso-river',
        type: 'line',
        source: 'seveso',
        paint: { 'line-color': '#1d4ed8', 'line-width': 2.5, 'line-opacity': 0.8 },
      })
    })
    mapRef.current = map
    return () => {
      mapRef.current = null
      map.remove()
    }
  }, [])

  // Hazard toggle: flips the PGRA layers' visibility (no-op until the style is loaded —
  // the layers are created visible and the checkbox starts checked, so they agree).
  useEffect(() => {
    const map = mapRef.current
    if (!map || !map.getLayer(pgraLayerId('P3'))) return
    for (const { scenario } of PGRA_SCENARIOS) {
      map.setLayoutProperty(pgraLayerId(scenario), 'visibility', showHazard ? 'visible' : 'none')
    }
  }, [showHazard])

  // Markers: rebuilt when the stations or the selection change (3 markers — cheap).
  useEffect(() => {
    const map = mapRef.current
    if (!map) return
    markersRef.current.forEach((m) => m.remove())
    markersRef.current = stations.map((s) => {
      const marker = new maplibregl.Marker({
        color: s.id === selectedId ? '#dc2626' : '#1d4ed8',
      })
        .setLngLat([s.lon, s.lat])
        .setPopup(new maplibregl.Popup({ offset: 24 }).setText(s.name))
        .addTo(map)
      marker.getElement().setAttribute('role', 'button')
      marker.getElement().setAttribute('aria-label', `Stazione ${s.name}`)
      marker.getElement().addEventListener('click', () => onSelect(s.id))
      return marker
    })
  }, [stations, selectedId, onSelect])

  return (
    <div className="map-wrap">
      <div ref={containerRef} className="map-view" aria-label="Mappa del Seveso" />
      <div className="map-legend">
        <label>
          <input
            type="checkbox"
            checked={showHazard}
            onChange={(e) => setShowHazard(e.target.checked)}
          />{' '}
          Aree allagabili (PGRA)
        </label>
        {showHazard && (
          <ul>
            {[...PGRA_SCENARIOS].reverse().map(({ scenario, label, color }) => (
              <li key={scenario}>
                <span className="legend-chip" style={{ backgroundColor: color }} /> {label}
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  )
}
