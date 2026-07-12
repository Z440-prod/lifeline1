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

## 3. Accounts, privacy & data rights (App Review blockers)
- [ ] Run all DB migrations on the production database — **including
      `006_accounts.sql`** (accounts + `account_devices`, RLS-locked).
- [ ] **Sign in with Apple** entitlement enabled in the iOS app (required by
      Guideline 4.8 because Google sign-in is offered). Configure the Apple +
      Google OIDC client credentials the backend verifies id-tokens against
      (`account/oauth` refuses unverified tokens in production by default).
- [ ] **Account deletion** verified end-to-end: Settings → Delete account →
      `DELETE /api/v1/account` erases account + all data (Guideline 5.1.1(v)).
- [ ] Privacy nutrition label / Data-safety form filled from
      `store/PRIVACY_LABELS.md` (note: **email is now collected** for sign-in).
- [ ] Privacy policy at `/privacy` mentions on-device AI + account email.

## 4. Store binaries (see `native/README.md`)
The native capabilities are **already coded** as the `lifeline-native` Capacitor
plugin (`native/plugins/lifeline-native`) and auto-wired by the web app — so
this phase is mostly "build, enable capabilities, sign, submit," not "write."
- [ ] `cd native && npm install && npx cap add ios && npx cap add android`, set signing, point `server.url` at production.
- [ ] `npm run assets` — generates all icons + splash from `native/assets/icon.svg`.
- [ ] `npx cap sync` — picks up the plugin + `@capacitor/local-notifications` + `@capacitor/device`.
- [ ] Create in-app subscriptions in App Store Connect & Play Console
      (`pro_monthly`, `elite_monthly`) — **store builds must use IAP, not Stripe** (Apple 3.1.1 / Play Payments policy). The `purchase` bridge → `POST /billing/store-receipt` path is already implemented end-to-end.
- [ ] Enable capabilities: iOS — In-App Purchase, HealthKit, App Attest, Sign in with Apple, Push; add `NSHealthShareUsageDescription` + `NSUserNotificationsUsageDescription`. (Stripe buttons already auto-hide inside the shell.)
- [ ] Wire the plugin's ⚙️ integration points you want (see plugin README): Google sign-in client ID, Health Connect query, and (optional) MediaPipe on-device AI + Wi-Fi-gated download.

## 4. Listings
- [ ] Copy from `store/LISTING.md`; privacy answers from `store/PRIVACY_LABELS.md`.
- [ ] Privacy policy URL: `https://<domain>/privacy` (already served).
- [ ] Screenshots: capture from the app in dark + light (6.7", 6.1", iPad, Android phone/tablet).
- [ ] Review notes from `store/REVIEW_NOTES.md`.

## 5. Final gates
- [ ] `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- [ ] Rate limits sized for launch (`[rate_limit]` in config).
- [ ] `/metrics` scraped by Prometheus; alerts on 5xx and webhook failures.
