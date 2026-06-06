// Fake maplibre-gl for jsdom (no WebGL there): records constructed maps and markers so
// tests can assert on them and fire map events (e.g. 'load') by hand. Use with:
// vi.mock('maplibre-gl', async () => (await import('../test/maplibre-mock')).maplibreMock())

import { vi } from 'vitest'

export const createdMarkers: FakeMarker[] = []
export const createdMaps: FakeMap[] = []

export class FakeMarker {
  options: { color?: string }
  lngLat: [number, number] | null = null
  remove = vi.fn()
  private el = document.createElement('div')

  constructor(options: { color?: string } = {}) {
    this.options = options
    createdMarkers.push(this)
  }
  setLngLat(ll: [number, number]) {
    this.lngLat = ll
    return this
  }
  setPopup() {
    return this
  }
  addTo() {
    return this
  }
  getElement() {
    return this.el
  }
}

export class FakeMap {
  handlers: Record<string, () => void> = {}
  layers: string[] = []
  addControl = vi.fn()
  addSource = vi.fn()
  remove = vi.fn()
  setLayoutProperty = vi.fn()

  constructor() {
    createdMaps.push(this)
  }
  on(event: string, handler: () => void) {
    this.handlers[event] = handler
  }
  addLayer(layer: { id: string }) {
    this.layers.push(layer.id)
  }
  getLayer(id: string) {
    return this.layers.includes(id) ? { id } : undefined
  }
  /** Test helper: simulate the style finishing loading. */
  fireLoad() {
    this.handlers['load']?.()
  }
}

class FakePopup {
  setText() {
    return this
  }
}

class FakeNavigationControl {}

export function maplibreMock() {
  return {
    default: {
      Map: FakeMap,
      Marker: FakeMarker,
      Popup: FakePopup,
      NavigationControl: FakeNavigationControl,
    },
  }
}
