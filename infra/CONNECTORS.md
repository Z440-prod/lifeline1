# Provisioned infrastructure & connector map

What each connected service does for Lifeline, what has already been
provisioned in your accounts, and what stays deliberately unused.

## ✅ Stripe — revenue (provisioned)

Created in the **Lifeline sandbox** account (`acct_1TrNFgV05D55xsPf`, test
mode — mirror the same three objects in live mode when you flip the switch):

| Object | ID |
|---|---|
| Product **Lifeline Pro** ($7.99/mo) | `prod_UrfAddv8Dg8qXk` |
| → price (wired into `config/default.toml`) | `price_1TrvpnV05D55xsPf83oNFGNb` |
| Product **Lifeline Elite** ($14.99/mo) | `prod_UrfADToaSn3rBh` |
| → price (wired into config) | `price_1TrvpyV05D55xsPfbURkZaBD` |
| Donation price (custom $1–$500, preset $5) | `price_1Trvq6V05D55xsPfbkTpDa8P` |
| Donation **Payment Link** (wired into config) | `https://donate.stripe.com/test_cNi4gC9Y6aB6eJK0lhcQU00` |

Still needed to take real money (2 minutes in the dashboard):
`ANTIGRAVITY__BILLING__STRIPE_SECRET_KEY` (sk_test to try; sk_live to launch)
and a webhook → `/api/v1/billing/webhook` → `…STRIPE_WEBHOOK_SECRET`.

## ✅ Supabase — production Postgres (provisioned, $0/month)

Project **`lifeline-engine`** (`mvupggovvuembwgdrtnx`, us-east-1,
free tier confirmed at $0/month). All five migrations applied:
`initial_schema`, `audit_logs`, `integrations`, `gamification_billing`,
plus `lock_down_postgrest_rls` — RLS enabled with **no policies** on every
table so Supabase's auto-generated REST/GraphQL API can read nothing; the
engine's direct Postgres connection is the only path to the data. The
security advisor reports only the expected INFO-level notices.

Point the engine at it (password is in your Supabase dashboard → Database):

```
ANTIGRAVITY__DATABASE__URL=postgresql://postgres.mvupggovvuembwgdrtnx:[PASSWORD]@aws-0-us-east-1.pooler.supabase.com:6543/postgres
```

Use the pooled (6543) URL — the engine's sqlx pool plays well with pgbouncer
and the free tier's connection limits.

## 🗺 Platform map (fast + cheap)

- **Engine (Rust binary)**: one small VM/container (Fly.io, Railway,
  Hetzner…). It serves the API *and* the web app, pre-compressed (brotli/
  gzip) with edge-cache headers, so a single tiny instance goes far.
- **Cloudflare** (connected): put it in front of the engine for free TLS,
  CDN caching (the engine already emits `Cache-Control` on rulebooks and
  assets, so Cloudflare serves repeats without touching your server), and
  DDoS protection. No Workers rewrite needed.
- **Vercel** (connected): optional home for a marketing/landing site;
  the product app itself is served by the engine.
- **iOS / Android**: Capacitor shells in `native/` (thin-client mode) with
  StoreKit/Play Billing → `/billing/store-receipt`.

## 🚫 Deliberately unused (by design, not omission)

- **Clerk** (auth): Lifeline has **no accounts** — identity is a
  hardware-attested device key. Adding account auth would weaken the
  zero-knowledge story and add cost.
- **Resend** (email): the app collects **no email addresses**. If you later
  want a marketing list on the landing page, Resend is ready — keep it out
  of the product itself.

## 💰 Revenue model (already wired)

Free (acquisition) → **Pro $7.99/mo** (the volume tier: all sources fused,
competition, unlimited coach) → **Elite $14.99/mo** (beta access, anchor) →
plus one-time **donations**. Web pays via Stripe; store builds via IAP.
All entitlements are enforced server-side, so every paid feature actually
requires paying.
