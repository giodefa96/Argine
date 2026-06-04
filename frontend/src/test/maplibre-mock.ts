// Fake maplibre-gl for jsdom (no WebGL there): records constructed markers so tests can
// assert on them. Use with: vi.mock('maplibre-gl', async () => (await import('../test/maplibre-mock')).maplibreMock())

import { vi } from 'vitest'

export const createdMarkers: FakeMarker[] = []

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

class FakeMap {
  on = vi.fn()
  addControl = vi.fn()
  addSource = vi.fn()
  addLayer = vi.fn()
  remove = vi.fn()
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
