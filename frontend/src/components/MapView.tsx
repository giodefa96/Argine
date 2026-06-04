// Map of the Seveso: river course (static GeoJSON from OSM) + station markers.
// Clicking a marker selects that station for the charts. Free OSM raster tiles with
// the required attribution; no API key (see frontend/docs/features/map-view.md).

import { useEffect, useRef } from 'react'
import maplibregl from 'maplibre-gl'
import 'maplibre-gl/dist/maplibre-gl.css'
import type { Station } from '../lib/api'
import sevesoRiver from '../assets/seveso-river.json'

const OSM_STYLE: maplibregl.StyleSpecification = {
  version: 8,
  sources: {
    osm: {
      type: 'raster',
      tiles: ['https://tile.openstreetmap.org/{z}/{x}/{y}.png'],
      tileSize: 256,
      attribution: '© OpenStreetMap contributors',
    },
  },
  layers: [{ id: 'osm', type: 'raster', source: 'osm' }],
}

// Upper Seveso basin → Milan, roughly centered between the stations.
const INITIAL_CENTER: [number, number] = [9.15, 45.62]
const INITIAL_ZOOM = 10

export interface MapViewProps {
  stations: Station[]
  selectedId: number | null
  onSelect: (id: number) => void
}

export default function MapView({ stations, selectedId, onSelect }: MapViewProps) {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const mapRef = useRef<maplibregl.Map | null>(null)
  const markersRef = useRef<maplibregl.Marker[]>([])

  // The map itself: created once, river layer added on load.
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

  return <div ref={containerRef} className="map-view" aria-label="Mappa del Seveso" />
}
