// Registers @testing-library/jest-dom matchers (toBeInTheDocument, etc.) with Vitest's expect,
// starts the MSW mock server, and resets DOM + request handlers between tests.
import '@testing-library/jest-dom/vitest'
import { cleanup } from '@testing-library/react'
import { afterAll, afterEach, beforeAll } from 'vitest'
import { server } from './server'

beforeAll(() => server.listen({ onUnhandledRequest: 'error' }))

afterEach(() => {
  cleanup()
  server.resetHandlers()
})

afterAll(() => server.close())
