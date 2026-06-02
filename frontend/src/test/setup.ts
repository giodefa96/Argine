// Registers @testing-library/jest-dom matchers (toBeInTheDocument, etc.) with Vitest's expect,
// and unmounts rendered components between tests so the DOM doesn't accumulate.
import '@testing-library/jest-dom/vitest'
import { cleanup } from '@testing-library/react'
import { afterEach } from 'vitest'

afterEach(() => {
  cleanup()
})
