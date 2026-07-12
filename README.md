<div align="center">

# 🫀 Lifeline

### The zero-knowledge longevity app — your health, computed on your device, ranked worldwide.

[![Rust](https://img.shields.io/badge/engine-Rust%20%2B%20Axum-orange.svg)](https://www.rust-lang.org/)
[![Frontend](https://img.shields.io/badge/app-vanilla%20JS%20PWA-f7df1e.svg)]()
[![Native](https://img.shields.io/badge/native-Capacitor%20(iOS%20%2B%20Android)-119eff.svg)]()
[![Privacy](https://img.shields.io/badge/architecture-zero--knowledge%20E2EE-2ea44f.svg)]()
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

</div>

---

## What is Lifeline?

Lifeline turns your body's signals — heart rate, HRV, sleep, movement, lab
results — into **one daily vitality score**, a biological **"Lifeline Age,"** a
cross-source **readiness** number, and the habits that actually move them. Then
it does something no other health app does: it lets you **compete on your health
itself**, on a global leaderboard the server ranks *blind*.

The whole thing is **zero-knowledge by construction**. Your raw health data is
computed on your device, from rule tables the server publishes. The server never
sees a heartbeat — only encrypted blobs it cannot read and a single opaque 0–100
integer you *choose* to share. On premium phones the AI coach even runs a local
**Gemma** model, so the app works with **no internet at all**.

One Rust binary — the **Antigravity engine** — is the entire product: it exposes
the API *and* serves the web app, which the iOS/Android shells wrap for the
stores.

---

## Highlights

| | |
|---|---|
| 🧬 **Lifeline Age** | A transparent, additive biological-age model — inspect every year it adds or subtracts. |
| ⚡ **Cross-source readiness** | Apple Health, Health Connect, and Whoop fuse into one readiness score, renormalized over whatever you've connected. |
| 🎚️ **The Conductor** | The app's rhythm adapts daily to your own readiness + habits: accent color, lead view, primary action, and the coach's tone all shift between **Recover / Maintain / Push** modes — computed on-device. |
| 🏆 **The Arena** | Log your opaque vitality score to a global ladder: six leagues (Bronze → Apex), weekly seasons, streaks, XP. Rivals see a handle and a number — never a biometric. |
| 🤖 **Device-adaptive AI coach** | The coach picks the best engine for the phone: **premium devices** (8 GB+, capable CPU/GPU) download an on-device **Gemma** model and answer fully offline; every other device uses a **cheaper open-source model** (Llama/Qwen/DeepSeek via any OpenAI-compatible endpoint) through the identity-stripping proxy — or Claude. Configurable via `[ai] provider`. Hard token budgets keep costs bounded. |
| 🔐 **The Vault** | Journals and lab results are AES-GCM-encrypted on-device and signed; the server stores ciphertext it can never decrypt. Labs are plotted against reference ranges locally. |
| 💳 **Subscriptions & donations** | Free / Pro / Elite tiers, enforced **server-side**. Stripe on the web; native IAP (StoreKit / Play Billing) in the store apps. |
| 👤 **Accounts, done privately** | Email + password (PBKDF2), Sign in with Apple, or Google — an identity layer that holds **no keys and no health data**. Delete it and everything tied to it, in-app. |
| 📴 **Offline-first** | A service worker precaches the shell + rule tables; with an on-device model installed, the app needs no network. |

---

## Architecture

```
┌────────────────────────────────────────────────────────────────────┐
│  iOS shell (Capacitor)        Android shell (Capacitor)             │
│      └──────────────┬───────────────────┘                          │
│                     │  thin client: loads the web app over TLS      │
│              ┌──────▼───────────────────────────────────────┐       │
│              │  Web app  (web/ — vanilla JS PWA)             │       │
│              │  • on-device insights engine (engine.js)     │       │
│              │  • WebCrypto E2EE (P-256 sign + AES-GCM)      │       │
│              │  • device scanner + on-device AI (Gemma)      │       │
│              │  • service worker (offline)                   │       │
│              └──────┬───────────────────────────────────────┘       │
│                     │  HTTPS  /api/v1/*                              │
│              ┌──────▼───────────────────────────────────────┐       │
│              │  Antigravity engine  (Rust · Axum · Tokio)    │       │
│              │  • serves the web app AND the API             │       │
│              │  • App Attest · sessions · rate limiting      │       │
│              │  • ships RULES, never computes on health data │       │
│              │  • Stripe + IAP receipt verification          │       │
│              └──────┬───────────────────────────────────────┘       │
│                     │                                                │
│              ┌──────▼──────┐   (falls back to in-memory mock if      │
│              │ PostgreSQL  │    unavailable — great for local dev)   │
│              └─────────────┘                                         │
└────────────────────────────────────────────────────────────────────┘
```

**The zero-knowledge contract.** The server publishes *rules* — band tables,
model coefficients, reference ranges, the Conductor's thresholds, the AI
policy matrix. The client applies them to plaintext health data that never
leaves the device. The only things the server ever stores are: client-side
encrypted vault blobs, a pseudonymous Arena score, subscription state, and an
account email for sign-in.

---

## Quick start

Requires the Rust toolchain. PostgreSQL is optional — without it the engine
falls back to an in-memory mock, which is perfect for local development.

```bash
cargo run
# → Antigravity engine listening on http://0.0.0.0:8443
# open http://127.0.0.1:8443  — the full app is served by the binary
```

For local development, run in dev mode so the browser can mint a session
(`/auth/dev-session`) and the sign-in gate can simulate Apple/Google:

```bash
ANTIGRAVITY__AUTH__ENVIRONMENT=development cargo run
```

### Build, run, and drive it

There's a ready-made skill that builds, launches, and end-to-end **drives** the
app (sign-up → portrait → coach → vault → settings, with screenshots):

```bash
# see .claude/skills/run-lifeline/SKILL.md
node .claude/skills/run-lifeline/driver.mjs
```

### Tests & quality gates

```bash
cargo test                                    # unit + integration
cargo clippy --all-targets -- -D warnings     # lint (warnings are errors)
cargo fmt --check                             # formatting
```

---

## Security model

- **Apple App Attest** — hardware-backed device identity; EC P-256 keys, strict
  monotonic replay protection. Browsers use a dev-only session mint (hard-disabled
  in production); accounts provide the production browser session.
- **End-to-end encryption** — the Vault is AES-256-GCM ciphertext with per-doc
  IV + auth tag, signed with the device's P-256 key (WebCrypto in the browser,
  Secure Enclave on device). The server verifies signatures against the
  registered public key and stores opaque blobs.
- **Passwords** — PBKDF2-HMAC-SHA256, 600k iterations, constant-time verify.
  Login never reveals whether an email exists.
- **OAuth** — Apple/Google id-tokens verified server-side (refused unverified in
  production by default).
- **Transport & headers** — TLS everywhere; CSP, HSTS (prod), `nosniff`,
  frame `DENY`, referrer/permissions policies on every response; Brotli/gzip.
- **Abuse & cost control** — per-IP token-bucket rate limiting; three-gate AI
  budget (per-device daily + monthly caps and a global daily circuit breaker).
- **Secrets** — never hardcoded; injected via `ANTIGRAVITY__*` env vars.
- **Data rights** — in-app **account deletion** (`DELETE /api/v1/account`)
  transactionally erases the account and all associated data.

See [`web/privacy.html`](web/privacy.html) for the user-facing policy and
[`store/PRIVACY_LABELS.md`](store/PRIVACY_LABELS.md) for the App Store / Play
data-safety answers.

---

## Project structure

```
src/                  Antigravity engine (Rust)
  routes/             API handlers (auth, account, sync, ai, game, billing, insights, integrations)
  crypto/             attestation, assertion, sessions, password (PBKDF2), token vault, oauth state
  db/                 Database trait + Postgres impl + in-memory MockDatabase
  models/             domain types (device, account, sync doc, game profile, subscription)
  middleware/         attest_guard (session verification)
migrations/           PostgreSQL schema (attested devices, sync, gamification, billing, accounts)
config/               default.toml + Apple App Attest root CA
web/                  the app the engine serves
  index.html          shell
  sw.js               offline service worker
  assets/             app.js/.css, api.js, engine.js (on-device insights),
                      charts.js, sound.js, device.js (scanner), localai.js (on-device AI)
  privacy.html        privacy policy (served at /privacy)
native/               Capacitor shells (iOS + Android) + bridge docs
store/                App Store / Play listing, privacy labels, review notes, launch checklist
tests/                integration tests (end-to-end over the router)
scripts/              load test + PGO build helpers
.claude/skills/       run-lifeline: build/launch/drive skill
```

---

## API surface (v1)

Everything is under `/api/v1`. A selection:

| Method & path | Purpose |
|---|---|
| `GET /auth/challenge` · `POST /auth/verify-attestation` · `POST /auth/assert` | App Attest device registration + assertion |
| `POST /account/register` · `/account/login` · `/account/oauth` | Sign up / in (email/password, Apple, Google) |
| `DELETE /account` | Permanent account + data deletion |
| `POST /sync/delta` · `GET /sync/document/{id}` | E2EE document sync (ciphertext only) |
| `POST /ai/proxy` | Identity-stripped coach proxy (budget-enforced) |
| `GET /ai/policy-matrix` · `GET /ai/local-models` | Coach policy + on-device model catalog |
| `GET /insights/config` | Rules for the on-device engine (incl. the Conductor) |
| `POST /game/score` · `GET /game/leaderboard` · `GET /game/profile` | The Arena |
| `POST /billing/checkout` · `/billing/webhook` · `/billing/store-receipt` | Stripe + native IAP |
| `GET /integrations` · `POST /integrations/{provider}/connect` | Apple/Google/Whoop sources |
| `GET /health` · `GET /metrics` | Liveness + Prometheus metrics |

---

## Deploying to the App Store & Google Play

The store apps are thin Capacitor shells that load the deployed web app over
TLS, so web releases reach both stores without resubmission. The full,
step-by-step path lives in:

- **[`store/LAUNCH_CHECKLIST.md`](store/LAUNCH_CHECKLIST.md)** — backend → Stripe → binaries → listings → final gates
- **[`native/README.md`](native/README.md)** — Capacitor build, App Attest, HealthKit, IAP + on-device AI bridge
- **[`store/LISTING.md`](store/LISTING.md)** · **[`store/PRIVACY_LABELS.md`](store/PRIVACY_LABELS.md)** · **[`store/REVIEW_NOTES.md`](store/REVIEW_NOTES.md)**

Compliance highlights already handled in-app: Sign in with Apple (4.8),
in-app account deletion (5.1.1(v)), server-side entitlement enforcement, native
IAP for store builds (3.1.1), and on-device model weights treated as data (2.5.2).

---

## Tech stack

**Engine:** Rust · Axum · Tokio · SQLx (PostgreSQL) · ring / RustCrypto · moka
(in-process cache) · tower-governor (rate limiting) · axum-prometheus.
**App:** vanilla JS (no framework, instant first paint) · WebCrypto · hash
router · service worker · Canvas/SVG charts.
**Native:** Capacitor (iOS + Android).
**Payments:** Stripe (web) · StoreKit / Play Billing (native).
**AI:** Claude via proxy · on-device Gemma (MediaPipe LLM / Core ML).

---

<div align="center">
<sub>Zero-knowledge by design. Your body, drawn fresh every morning — and never shared without your say-so.</sub>
</div>
