## Agent skills

### Issue tracker

GitHub Issues via the `gh` CLI. PRs are not a triage surface.
See `docs/agents/issue-tracker.md`.

### Triage labels

Default vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`).
See `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout — one `CONTEXT.md` + `docs/adr/` at the repo root.
See `docs/agents/domain.md`.

### Code

Always apply SOLID principles when writing or refactoring code.
Use test-driven development (TDD): write a failing test first, implement the smallest change to pass it, then refactor.

