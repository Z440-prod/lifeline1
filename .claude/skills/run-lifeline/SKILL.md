---
name: run-lifeline
description: Build, launch, and drive the Lifeline app (Antigravity Rust/Axum engine + web app it serves). Use to run, start, serve, smoke-test, screenshot, or end-to-end drive Lifeline — sign-in, portrait, coach, vault, on-device AI, offline. Also covers the API smoke path via curl.
---

# Run Lifeline

Lifeline is a Rust/Axum backend (the "Antigravity engine") that **also serves the
vanilla-JS web app** from its own root. So one binary is the whole product:
`cargo run` gives you the API *and* the app at `http://127.0.0.1:8443`.

Two ways to drive it, both verified in this container:

- **Web UI** — a headless-Chromium driver at
  `.claude/skills/run-lifeline/driver.mjs` boots the app and walks the real
  flow (onboarding → sign-up → portrait → coach → vault → settings), screenshotting
  each step and failing on any console error. **This is the primary agent path.**
- **API** — `curl` against `/api/v1/*` for backend-only checks.

All paths below are relative to the repo root (`/home/user/lifeline1`).

## Prerequisites

Rust toolchain (cargo) is already present. The browser driver needs the
container's preinstalled Chromium + Playwright — no `npm install`, no
`playwright install`:

```bash
# Chromium lives here; Playwright is in the Node global modules.
ls /opt/pw-browsers/chromium
ls /opt/node22/lib/node_modules/playwright/index.js
```

The driver imports Playwright by that absolute path. If yours differs, set
`PLAYWRIGHT_MODULE=$(npm root -g)/playwright/index.js` and
`CHROMIUM_PATH=/path/to/chromium`.

## Build

```bash
cargo build
```

Postgres is optional: with no reachable database the engine logs a warning and
falls back to an in-memory `MockDatabase`, which is exactly what you want for a
local run.

## Run (agent path)

Start the server in the background, then run the driver against it.

```bash
# 1. Launch the engine (dev mode enables /auth/dev-session so the browser can
#    mint a session; it also lets the sign-up gate simulate Apple/Google).
ANTIGRAVITY__AUTH__ENVIRONMENT=development ./target/debug/antigravity \
  > /tmp/lifeline-server.log 2>&1 &

# 2. Wait for it to listen (port comes from config/default.toml → 8443).
sleep 2 && curl -s http://127.0.0.1:8443/health   # {"service":"antigravity","status":"ok",...}

# 3. Drive the web app. Screenshots land in ./.driver-shots/ (01-gate … 05-settings).
node .claude/skills/run-lifeline/driver.mjs
```

Expected tail:

```
✓ load + skip onboarding
✓ sign up (email + password)
✓ portrait shows a vitality score + Conductor banner
✓ coach replies (proxy or on-device)
✓ vault stores encrypted journal
✓ settings shows account + on-device AI card
✓ service worker registered (offline shell cached)

console errors: none
ALL STEPS PASSED
```

The driver takes optional args: `node .claude/skills/run-lifeline/driver.mjs [baseUrl] [outDir]`.

### Driving on-device AI (premium-device path)

The coach runs a local model on eligible devices. Headless Chromium usually
reads as a "premium" device (8 GB + WebGPU) so the **offer** appears, but to
exercise the real native-bridge path (download → ready → on-device replies),
inject a fake bridge before load — the same shape the iOS/Android shell provides:

```js
await ctx.addInitScript(() => {
  window.Capacitor = { isNativePlatform: () => true };
  window.LifelineDevice = { profile: { ram_gb: 8, cores: 8, os: 'ios', has_npu: true, ai_backends: ['native-mediapipe'] } };
  let ready = false;
  window.LifelineLocalAI = {
    isReady: () => ready,
    download: async (id, onp) => { for (let p = 0; p <= 100; p += 20) { onp(p); await new Promise(r => setTimeout(r, 40)); } ready = true; },
    generate: async () => 'On-device reply. (native)',
    remove: async () => { ready = false; },
  };
});
```

## Run (API smoke, no browser)

```bash
B=http://127.0.0.1:8443
curl -s $B/api/v1/ai/local-models | head -c 200          # on-device model catalog
DEV=$(python3 -c "import uuid;print(uuid.uuid4())")
curl -s -o /dev/null -w "register:%{http_code}\n" -X POST $B/api/v1/account/register \
  -H 'content-type: application/json' \
  -d "{\"email\":\"smoke@lifeline.test\",\"password\":\"smokepass123\",\"device_id\":\"$DEV\"}"
curl -sD - -o /dev/null $B/ | grep -i content-security-policy   # security headers present
```

## Run (human path)

`cargo run` (foreground), then open `http://127.0.0.1:8443` in a real browser and
Ctrl-C to stop. Useless headless — use the driver above instead.

## Test

```bash
cargo test                       # 31 unit + 13 integration
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

## Gotchas

- **Port is 8443, not 8080.** It comes from `config/default.toml` (`server.port`),
  not a CLI flag. `curl localhost:8080` fails with connection-refused (curl exit 7).
- **The sign-up gate blocks boot.** On first load the app shows a sign-in/sign-up
  gate and `main()` awaits it. The driver must sign up (or the app never renders
  the tab bar). The service worker is registered *before* the gate, so offline
  caching still happens for signed-out users.
- **Playwright is CommonJS.** Importing it into an ESM `.mjs` puts the named
  exports under `.default`; the driver handles both (`pw.chromium ? pw : pw.default`).
  A bare `import 'playwright'` will not resolve — use the absolute path.
- **Rulebook cache is empty on the very first load.** The service worker only
  controls the page after it claims, so the first boot's config fetches go direct;
  they land in `lifeline-rules-v1` on the next navigation. The shell
  (`lifeline-shell-v1`) is precached on install, so the app opens offline
  immediately.
- **Dev vs prod session.** `ANTIGRAVITY__AUTH__ENVIRONMENT=development` is what
  enables `/auth/dev-session` and the simulated Apple/Google tokens
  (`sim:<subject>:<email>`). Without it the browser can't get a session and the
  gate can't complete.
- **`MockDatabase` is in-memory.** Every server restart wipes accounts, scores,
  and vault docs. That's expected for a local run.

## Troubleshooting

- `TypeError: Cannot read properties of undefined (reading 'launch')` — Playwright
  import returned the CJS namespace; use `(await import(PW)).default`. (Already
  handled in the driver.)
- Driver hangs on `#authPrimary` / `.tabbar` never appears — the server isn't in
  `development` mode, so the sign-up couldn't mint a session. Restart it with the
  `ANTIGRAVITY__AUTH__ENVIRONMENT=development` prefix.
- `curl: (7) Failed to connect` — server not up yet or wrong port; check
  `/tmp/lifeline-server.log` and use port **8443**.
