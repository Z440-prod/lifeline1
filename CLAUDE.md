# Lifeline — project workflow

Lifeline is a zero-knowledge E2EE health/longevity app: a Rust/Axum backend
("Antigravity engine") that also serves the vanilla-JS web app in `web/`, plus
Capacitor native shells (iOS/Android). One binary is the whole product.

## Everyday commands
```bash
cargo build                                   # build the engine
cargo test                                    # 51 unit + integration tests
cargo clippy --all-targets -- -D warnings     # lint (must be clean)
cargo fmt                                      # format
# Run + drive the app (dev mode enables /auth/dev-session):
ANTIGRAVITY__AUTH__ENVIRONMENT=development ./target/debug/antigravity   # serves http://127.0.0.1:8443
```
Server binds **8443** (from `config/default.toml`), not 8080. With no reachable
Postgres it falls back to an in-memory MockDatabase — expected for local runs.

## Project skills (in `.claude/skills/`)
- **run-lifeline** — build, launch, and browser-drive the app end-to-end.
- **appstore-readiness** — 9-agent iOS App Store submission audit.
- **ui-ux-pro-max** — design-intelligence database (styles, palettes, type, UX).

## Deploy & docs
`deploy/GO-LIVE.md` (runbook), `SECURITY_REVIEW.md`, `store/` (listing + audit),
`design-system/lifeline/MASTER.md` (persisted UI design system).

---

## gstack (recommended AI workflow)

This project uses [gstack](https://github.com/garrytan/gstack) for AI-assisted
workflows — a seven-stage sprint: **Think → Plan → Build → Review → Test → Ship
→ Reflect**. Install it for the best experience (the global install is not
committed here — it's 1.6 GB and gstack itself de-vendors project copies):

```bash
git clone --depth 1 https://github.com/garrytan/gstack.git ~/.claude/skills/gstack
cd ~/.claude/skills/gstack && ./setup --team
```

Skills like `/office-hours`, `/autoplan`, `/qa`, `/review`, `/ship`,
`/investigate`, `/cso` (security), and `/browse` become available after install.
Use `/browse` for all web browsing. Use `~/.claude/skills/gstack/...` for gstack
file paths.
