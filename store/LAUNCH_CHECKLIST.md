# Lifeline launch checklist

## 1. Backend to production
- [ ] Deploy the engine behind TLS (nginx/Caddy/cloud LB) — it speaks plain HTTP itself.
- [ ] `ENVIRONMENT=production` (this hard-disables `/auth/dev-session` — verified by test).
- [ ] Set real secrets via env (never commit):
      `ANTIGRAVITY__AUTH__SERVER_SECRET` (≥32 random bytes),
      `ANTIGRAVITY__AUTH__APPLE_TEAM_ID`, `ANTIGRAVITY__AUTH__APPLE_BUNDLE_ID`,
      `ANTIGRAVITY__AI__ANTHROPIC_API_KEY`,
      `ANTIGRAVITY__DATABASE__URL` (production Postgres).
- [ ] Point `billing.success_url` / `cancel_url` / `portal_return_url` at production pages or deep links.

## 2. Stripe (web subscriptions)
- [ ] In the Stripe dashboard (or via the Stripe MCP connector when linked), create:
      product **Lifeline Pro** with recurring monthly price **$7.99** →
      `ANTIGRAVITY__BILLING__PRICE_PRO=price_…`;
      product **Lifeline Elite** with recurring monthly price **$14.99** →
      `ANTIGRAVITY__BILLING__PRICE_ELITE=price_…`.
- [ ] `ANTIGRAVITY__BILLING__STRIPE_SECRET_KEY=sk_live_…`.
- [ ] Add a webhook endpoint → `https://<domain>/api/v1/billing/webhook`
      with events `checkout.session.completed`,
      `customer.subscription.updated`, `customer.subscription.deleted`;
      set `ANTIGRAVITY__BILLING__STRIPE_WEBHOOK_SECRET=whsec_…`.
- [ ] Enable the customer billing portal in Stripe settings.
- [ ] Test-mode dry run: checkout → webhook flips tier → beta endpoint opens.

## 3. Store binaries (see `native/README.md`)
- [ ] `npx cap add ios && npx cap add android`, set signing, point `server.url` at production.
- [ ] Create in-app subscriptions in App Store Connect & Play Console
      (`pro_monthly`, `elite_monthly`) — **store builds must use IAP, not Stripe** (Apple 3.1.1 / Play Payments policy).
- [ ] Implement `POST /billing/store-receipt` receipt validation feeding the same `upsert_subscription` (backend tier logic unchanged).
- [ ] Hide Stripe purchase buttons when `window.Capacitor` is defined.
- [ ] HealthKit / Health Connect permissions + usage strings.
- [ ] Adopt real App Attest in the iOS shell.

## 4. Listings
- [ ] Copy from `store/LISTING.md`; privacy answers from `store/PRIVACY_LABELS.md`.
- [ ] Privacy policy URL: `https://<domain>/privacy` (already served).
- [ ] Screenshots: capture from the app in dark + light (6.7", 6.1", iPad, Android phone/tablet).
- [ ] Review notes from `store/REVIEW_NOTES.md`.

## 5. Final gates
- [ ] `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- [ ] Rate limits sized for launch (`[rate_limit]` in config).
- [ ] `/metrics` scraped by Prometheus; alerts on 5xx and webhook failures.
