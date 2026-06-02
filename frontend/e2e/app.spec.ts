import { test, expect } from '@playwright/test'

// Critical journey: the app loads and shows the landing content.
test('landing page shows Argine and the Seveso', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('heading', { name: /argine/i })).toBeVisible()
  await expect(page.getByText(/seveso/i)).toBeVisible()
})
