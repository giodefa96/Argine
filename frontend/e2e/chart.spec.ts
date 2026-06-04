import { expect, test } from '@playwright/test'

// Critical journey: the map + level chart render with data. The backend is mocked at the
// network layer (Playwright route) so the E2E needs no running API; OSM tile requests are
// stubbed too — no real external call in CI (CLAUDE.md).
test('renders the Seveso map and level chart with data', async ({ page }) => {
  await page.route('https://tile.openstreetmap.org/**', (route) =>
    route.fulfill({ status: 204, body: '' }),
  )
  await page.route('**/stations', (route) =>
    route.fulfill({
      json: [
        {
          id: 1,
          source: 'arpa_lombardia',
          external_id: '3118',
          name: 'Milano Niguarda',
          river: 'Seveso',
          kind: 'hydrometric',
          lat: 45.5258,
          lon: 9.1922,
          created_at: '2026-06-03T00:00:00Z',
        },
      ],
    }),
  )
  await page.route('**/stations/*/observations*', (route) => {
    const isRain = new URL(route.request().url()).searchParams.get('metric') === 'rain_mm'
    route.fulfill({
      json: isRain
        ? [
            { station_id: 1, ts: '2026-06-02T09:00:00Z', metric: 'rain_mm', value: 5.0 },
            { station_id: 1, ts: '2026-06-02T10:00:00Z', metric: 'rain_mm', value: 2.0 },
          ]
        : [
            { station_id: 1, ts: '2026-06-02T10:00:00Z', metric: 'level_m', value: 0.5 },
            { station_id: 1, ts: '2026-06-02T11:00:00Z', metric: 'level_m', value: 0.7 },
          ],
    })
  })

  await page.goto('/')

  // The map renders with one marker per station (markers are DOM elements, not canvas)
  // and the PGRA hazard legend with its toggle.
  await expect(page.getByLabel('Mappa del Seveso')).toBeVisible()
  await expect(page.locator('.maplibregl-marker')).toHaveCount(1)
  await expect(page.getByRole('checkbox', { name: /aree allagabili/i })).toBeChecked()
  await expect(page.getByText('P3 — frequente')).toBeVisible()

  // The station selector is populated from the API (options live inside a closed <select>,
  // so assert on the selected value rather than option visibility).
  await expect(page.getByRole('combobox', { name: /stazione/i })).toHaveValue('1')
  await expect(page.getByText(/Ultimo livello/)).toBeVisible()
  // uPlot draws a real canvas inside the chart container.
  await expect(page.locator('[data-testid="level-chart"] canvas')).toBeVisible()
})
