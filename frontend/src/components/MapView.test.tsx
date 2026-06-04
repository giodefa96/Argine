import { fireEvent, render, screen } from '@testing-library/react'
import { beforeEach, expect, test, vi } from 'vitest'
import type { Station } from '../lib/api'
import { createdMarkers } from '../test/maplibre-mock'
import MapView from './MapView'

// jsdom has no WebGL — see src/test/maplibre-mock.ts.
vi.mock('maplibre-gl', async () => (await import('../test/maplibre-mock')).maplibreMock())

const station = (id: number, name: string): Station => ({
  id,
  source: 'arpa_lombardia',
  external_id: String(id),
  name,
  river: 'Seveso',
  kind: 'hydrometric',
  lat: 45.5 + id / 100,
  lon: 9.1 + id / 100,
  created_at: '2026-01-01T00:00:00Z',
})

const stations = [station(1, 'Cantù Asnago'), station(2, 'Milano Niguarda')]

beforeEach(() => {
  createdMarkers.length = 0
})

test('renders the map container with one marker per station', () => {
  render(<MapView stations={stations} selectedId={null} onSelect={() => {}} />)
  expect(screen.getByLabelText('Mappa del Seveso')).toBeInTheDocument()
  expect(createdMarkers).toHaveLength(2)
  expect(createdMarkers[0].lngLat).toEqual([stations[0].lon, stations[0].lat])
})

test('highlights the selected station', () => {
  render(<MapView stations={stations} selectedId={2} onSelect={() => {}} />)
  expect(createdMarkers[0].options.color).toBe('#1d4ed8')
  expect(createdMarkers[1].options.color).toBe('#dc2626')
})

test('clicking a marker selects its station', () => {
  const onSelect = vi.fn()
  render(<MapView stations={stations} selectedId={null} onSelect={onSelect} />)
  fireEvent.click(createdMarkers[1].getElement())
  expect(onSelect).toHaveBeenCalledWith(2)
})

test('markers are accessible buttons named after the station', () => {
  render(<MapView stations={stations} selectedId={null} onSelect={() => {}} />)
  expect(createdMarkers[0].getElement()).toHaveAttribute('aria-label', 'Stazione Cantù Asnago')
  expect(createdMarkers[0].getElement()).toHaveAttribute('role', 'button')
})
