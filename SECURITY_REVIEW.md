# Lifeline — security weak-spot review

Manual weak-spot audit of the whole app, grounded in the **OWASP Top 10 (2021)**
and **OWASP ASVS**. Focus on the newest surfaces (adaptive AI provider, Stripe
Payment Links, waitlist site) plus a full sweep. Date: 2026-07-12.

> Guidelines and threats evolve — re-run before each release. Backed by
> `cargo clippy -D warnings`, `cargo test` (51 tests), and targeted source sweeps.

## Findings

### 🔴 FIXED — Payment tier could be spoofed via the checkout URL (A04 / A08)
**Was:** the Stripe **webhook** granted the tier from the `__tier` suffix of
`client_reference_id`. In a Payment Link checkout that value is a URL parameter
the payer controls, so someone could open the **Pro** link ($7.99), append
`?client_reference_id=<device>__elite`, pay Pro, and receive **Elite**.
**Fix:** the granted tier is now derived from `amount_total` — the amount the
customer **actually paid** — via `BillingConfig::tier_for_amount`. The
client-supplied tier is only a fallback when expected amounts aren't configured,
and a mismatch is logged. Expected prices are set in config
(`amount_pro_cents = 799`, `amount_elite_cents = 1499`). Unit-tested
(`tier_from_amount_is_spoof_proof`).

## Verified clean (no action needed)

| Area | Check | Result |
|---|---|---|
| **A01 Broken access control** | Entitlements (arena/coach/sources/history) gated server-side per device; admin endpoint disabled by default + constant-time token | ✅ |
| **A02 Crypto failures** | Vault AES-256-GCM + ECDSA client-side; passwords PBKDF2-HMAC-SHA256 600k; HMAC session tokens verified constant-time (ring) | ✅ |
| **A03 Injection (SQL)** | All DB access via sqlx parameterized queries — no string-built SQL anywhere in `src/` | ✅ |
| **A03 Injection (SSRF)** | Every outbound URL (Anthropic, open-source model, Whoop, Apple, Stripe) is **operator config**, never user input | ✅ |
| **A04 Insecure design** | AI coach has global + per-device daily/monthly token budgets; the payment-tier flaw above is the one that slipped, now fixed | ✅ |
| **A05 Misconfiguration** | Prod fail-fast guards (server_secret ≥32B non-placeholder, AI key, Apple team id, DB); admin + billing disabled until configured | ✅ |
| **A07 Auth failures** | App Attest / assertion guard; dev-session only in `environment=development`; sign-in via OIDC or PBKDF2 | ✅ |
| **A08 Integrity** | Stripe webhook verifies HMAC signature **and** a 5-min replay window; store receipts verified server-side | ✅ |
| **A09 Logging** | Audit-log rows on AI proxy, billing, deletion; aggregate-only admin stats (no PII) | ✅ |
| **Secrets** | No real secrets committed; the Stripe Payment Links in config are **public** buy URLs, not secrets; `.env.production` git-ignored | ✅ |
| **Waitlist site** | Firestore rules are **create-only, no reads** (list can't be read from a browser); email-shape validated; extra fields rejected; honeypot | ✅ |
| **AI open-source provider** | New `/ai/proxy` open-source path adds no user-controlled URL, no new client identifier; response normalized server-side | ✅ |
| **Transport / headers** | CSP, HSTS (prod), nosniff, frame DENY, strict Cache-Control via `harden_and_cache` | ✅ |

## Residual notes (accept / monitor)
- **Waitlist spam:** anyone can create many docs (one per distinct email). Fine
  for a waitlist; add Firebase **App Check** (reCAPTCHA) if abused — no form change.
- **Payment amount source:** if a Stripe event ever omits `amount_total`, the code
  falls back to the (now-configured) expected amounts; keep `amount_*_cents` in
  sync with your real Payment Link prices.
