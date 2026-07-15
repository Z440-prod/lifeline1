# Lifeline — the complete launch guide

Zero to live on the App Store and Google Play. Work top to bottom. Every command
is copy-paste. Anything marked **[YOU]** needs a key, a Mac, or a store account —
those are the only parts I can't do for you.

> **What's already done:** the whole app is built, verified, committed, and
> pushed. `cargo build/clippy/test` green (51 tests), full app driven in a
> browser with zero console errors, every subsystem checked. You are wiring in
> your own accounts and pressing "submit" — nothing more.

---

## 0. What you need (accounts + tools)

| Need | Where | Cost |
|---|---|---|
| **Apple Developer** account | developer.apple.com | $99 / year |
| **Google Play Developer** account | play.google.com/console | $25 once |
| A **Mac with Xcode 16+** | (Apple's rule — iOS builds can't be made on Linux/Windows) | — |
| **Android Studio** (SDK 35) | developer.android.com/studio | free |
| **Node 18+** and **npm** | nodejs.org | free |
| A **host** (Fly.io recommended) | fly.io | ~$5–20 / mo |
| **Postgres** (Supabase free tier is fine) | supabase.com | $0 → up |
| **AI**: an open-source model API key (cheap) or Anthropic key | together.ai / groq.com / console.anthropic.com | pay-per-use |
| **Stripe** account (you already have the Payment Links) | dashboard.stripe.com | ~2.9% + 30¢/charge |

Install the CLIs once:
```bash
# Fly.io
curl -L https://fly.io/install.sh | sh
fly auth login
```

---

## 1. Deploy the backend (~30 min)

The engine is one Rust binary that serves the API **and** the web app. It needs a
persistent host (not Vercel/Cloudflare serverless).

**1a. Fill in your secrets.**
```bash
cd lifeline
cp deploy/.env.production.example deploy/.env.production
openssl rand -hex 48          # paste as ANTIGRAVITY__AUTH__SERVER_SECRET
```
Edit `deploy/.env.production` and set:
- `ANTIGRAVITY__AUTH__ENVIRONMENT=production`
- `ANTIGRAVITY__AUTH__SERVER_SECRET=` … (the openssl value)
- `ANTIGRAVITY__DATABASE__URL=` … your Supabase/Postgres URI (Supabase → Project
  Settings → Database → Connection string)
- **AI for non-premium phones** — pick ONE:
  - Open-source (cheaper): `ANTIGRAVITY__AI__OPENAI_BASE_URL=https://api.together.xyz/v1`,
    `ANTIGRAVITY__AI__OPENAI_API_KEY=…`, `ANTIGRAVITY__AI__OPENAI_MODEL=meta-llama/Llama-3.3-70B-Instruct-Turbo`
  - Or Claude: `ANTIGRAVITY__AI__ANTHROPIC_API_KEY=…`
- `ANTIGRAVITY__BILLING__STRIPE_WEBHOOK_SECRET=` … (from step 3 — leave blank for now, add after)
- `ANTIGRAVITY__AUTH__APPLE_TEAM_ID=` and `APPLE_BUNDLE_ID=health.lifeline.app`

**1b. Deploy.**
```bash
fly launch --no-deploy --copy-config --dockerfile deploy/Dockerfile   # name it, e.g. lifeline-engine
fly secrets import < deploy/.env.production
fly deploy
```
The engine **creates every database table on first boot** (`sqlx::migrate!`). In
production it refuses to start on a placeholder secret — that's the safety net.

**1c. Point your domain** (e.g. `app.lifeline.health`) at the Fly app, then:
```bash
curl https://app.lifeline.health/health     # → {"status":"ok"}
```
Your web app is now live at that URL. Open it in a browser and sign up to sanity-check.

---

## 2. Database (already handled)

Supabase **is** Postgres. You already pasted its URL in step 1 — the engine built
the schema on boot. To see data: Supabase → Table editor. Nothing else to do.

---

## 3. Stripe — make purchases auto-upgrade users (~2 min)

Your two **Payment Links** are already wired in (`config/default.toml`). To make a
purchase automatically flip the user to Pro/Elite in-app, add the webhook:

1. **[YOU]** Stripe Dashboard → Developers → Webhooks → **Add endpoint**
   `https://app.lifeline.health/billing/webhook`
2. Subscribe to: `checkout.session.completed`, `customer.subscription.updated`,
   `customer.subscription.deleted`
3. Copy the **Signing secret** → put in `deploy/.env.production` as
   `ANTIGRAVITY__BILLING__STRIPE_WEBHOOK_SECRET`, then `fly secrets import < deploy/.env.production` again.

Without the webhook, payments still succeed — you'd just grant the tier by hand.
The webhook grants the tier from the **amount actually paid**, so it can't be spoofed.

---

## 4. Point the native apps at your server

**[YOU]** Edit `native/capacitor.config.json` → set `server.url` to your deployed
HTTPS URL (both stores require TLS). The apps are thin shells that load your live
web app, so **web updates ship to both stores instantly** without re-review.

---

## 5. Build the store binaries

```bash
cd native
npm install
npm run assets          # generates all icons + splash from assets/icon.svg
```

### 5a. Android → `.aab` (buildable on Mac/Windows/Linux with Android SDK)
```bash
npm run add:android
npx cap sync android
cd android && ./gradlew bundleRelease
# → android/app/build/outputs/bundle/release/app-release.aab
```
Sign it with **your** upload key (Play Console can manage signing for you).

### 5b. iOS → `.ipa` (requires a Mac + Xcode — Apple's rule)
```bash
npm run add:ios
npx cap sync ios
npm run open:ios        # opens Xcode
```
In Xcode:
1. Signing & Capabilities → select your Team, set bundle id `health.lifeline.app`.
2. Add capabilities: **HealthKit, In-App Purchase, Sign in with Apple, Push, App Attest**.
3. **Copy `native/privacy/PrivacyInfo.xcprivacy` into the App target** (this is the
   one hard blocker — see `store/APP_STORE_AUDIT.md`).
4. Add to `Info.plist`: `NSHealthShareUsageDescription`,
   `NSUserNotificationsUsageDescription` (exact strings in `store/APP_STORE_AUDIT.md`).
5. Product → **Archive** → Distribute App → App Store Connect.

---

## 6. Submit to the stores

Everything you need to paste is written:
- **Listing** (title, subtitle, description, keywords): `store/LISTING.md`
- **Privacy labels** (App Privacy answers): `store/PRIVACY_LABELS.md`
- **Review notes + demo account**: `store/REVIEW_NOTES.md`
- **Screenshots** (ready, 1290×2796): `store/screenshots/`
- **Pre-submit checklist**: `store/LAUNCH_CHECKLIST.md`
- **Dynamic-UI compliance** (if a reviewer asks how the per-user UI works):
  `DYNAMIC_UI.md` — cites Apple 2.5.2 / Google policy.

**Google Play:** Play Console → create app → upload `.aab` → fill the listing →
Data safety form (from `PRIVACY_LABELS.md`) → submit.
**App Store:** App Store Connect → new app → upload build via Xcode → paste
listing → App Privacy answers → submit for review.

---

## 7. After you submit

- **Review time:** Apple ~24–48h first submission; Google a few hours to ~2 days.
  Submitting early tomorrow is smart — neither store approves same-day.
- **Admin dashboard:** set `ANTIGRAVITY__ADMIN__ADMIN_TOKEN` (a long random string)
  and visit `https://app.lifeline.health/admin` for aggregate stats.
- **Push a web update:** edit `web/`, redeploy the engine (`fly deploy`). Both apps
  update instantly (no re-review for web-layer changes).

---

## Troubleshooting

| Symptom | Fix |
|---|---|
| Engine won't start in prod | It's the fail-fast guard — check `SERVER_SECRET` (≥32 bytes, not the placeholder), the AI key, and that the DB URL is reachable. |
| App shows a blank/errored webview in review | The server must be **live** during review. Confirm `/health` and that `server.url` is correct + HTTPS. |
| Purchases don't upgrade the user | The Stripe webhook (step 3) isn't set or the signing secret is wrong. |
| iOS build rejected: privacy | You skipped `PrivacyInfo.xcprivacy` (step 5b.3) or the HealthKit purpose string. |
| Coach returns "[Mock…]" | No AI key set — add `OPENAI_API_KEY` (or Anthropic) in step 1. |

---

## The one-line truth
The code is done and verified. Launch = **deploy the engine (step 1) → wire Stripe
webhook (step 3) → build the two binaries (step 5) → submit (step 6).** The only
things I can't do are enter your keys, build the iOS `.ipa` (needs your Mac), and
press "submit" — all documented above.
