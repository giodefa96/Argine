# Backend docs

Single source of truth for the **low-level** state of the Rust backend. If you want to
understand how the backend works, start here.

**Rule:** this folder must stay in sync with the code. Every feature gets its own document
under [`features/`](./features/). When you add or change a feature, update its doc in the
same PR — docs drift is treated as a bug.

## Index
- [`ARCHITECTURE.md`](./ARCHITECTURE.md) — modules, startup flow, config, endpoints, planned components.
- `features/` — one document per feature:
  - [`config-and-startup.md`](./features/config-and-startup.md)
  - [`health.md`](./features/health.md)

## Feature doc template
Each `features/<name>.md` should cover:
1. **Overview** — what it does, in one paragraph.
2. **Design** — how it works, decisions and trade-offs.
3. **Files / code** — the modules, functions, routes involved (`path:symbol`).
4. **Contract** — inputs/outputs, env vars, API shape, errors.
5. **Status** — implemented / planned / partial.
6. **Open questions** — what's undecided.
