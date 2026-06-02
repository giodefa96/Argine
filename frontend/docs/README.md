# Frontend docs

Single source of truth for the **low-level** state of the React frontend. Start here to
understand how the frontend works.

**Rule:** this folder must stay in sync with the code. Every feature gets its own document
under [`features/`](./features/). When you add or change a feature, update its doc in the
same PR — docs drift is treated as a bug.

## Index

- [`ARCHITECTURE.md`](./ARCHITECTURE.md) — tooling, entry flow, structure, planned components.
- `features/` — one document per feature:
  - [`app-shell.md`](./features/app-shell.md)

## Feature doc template

Each `features/<name>.md` should cover:

1. **Overview** — what it does, in one paragraph.
2. **Design** — how it works, decisions and trade-offs.
3. **Files / code** — components, hooks, routes involved (`path:symbol`).
4. **Contract** — props, state, API calls, routes.
5. **Status** — implemented / planned / partial.
6. **Open questions** — what's undecided.
