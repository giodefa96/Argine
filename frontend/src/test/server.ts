// MSW node server for tests (Vitest). E2E uses Playwright route mocking instead.
import { setupServer } from 'msw/node'
import { handlers } from './handlers'

export const server = setupServer(...handlers)
