# Lifeline native shells (iOS + Android)

Capacitor wrappers that put the Lifeline web app (`../web`, served by the
Antigravity engine) into App Store / Google Play binaries. The config runs in
**thin-client mode**: the shells load the deployed web app over TLS, so web
releases reach both stores instantly without resubmission.

## Build steps

```bash
cd native
npm install
npx cap add ios        # requires Xcode 16+ on macOS
npx cap add android    # requires Android Studio (SDK 35)
npx cap sync
npx cap open ios       # set signing team, bundle id health.lifeline.app
npx cap open android   # set signing config, applicationId health.lifeline.app
```

Before building, point `server.url` in `capacitor.config.json` at your
production deployment (must be HTTPS; both platforms reject cleartext).

## iOS specifics

- **App Attest**: the native shell should adopt the real `DCAppAttestService`
  flow against `/api/v1/auth/challenge` → `/verify-attestation` → `/assert`.
  The backend is already production-ready for it; set
  `ANTIGRAVITY__AUTH__APPLE_TEAM_ID` / `APPLE_BUNDLE_ID` and
  `ENVIRONMENT=production` (which also hard-disables `/auth/dev-session`).
- **HealthKit**: add the HealthKit capability + `NSHealthShareUsageDescription`.
  Signals are read on-device and fed to the same insights engine the web app
  uses (`web/assets/engine.js` mirrors the server's published rules).

## ⚠️ Subscriptions in the store builds

Apple (App Review 3.1.1) and Google (Play Payments policy) both require
**in-app digital subscriptions to use their own billing** — Stripe Checkout is
for the web app only and must not be reachable as a purchase path inside the
store binaries.

The backend is already shaped for this: entitlements hang off a `tier` per
device, and Stripe is just one writer of that state. Launch plan:

1. Create matching subscription products in App Store Connect and Play Console
   (`pro_monthly` $7.99, `elite_monthly` $14.99).
2. In the shells, purchase via StoreKit 2 / Play Billing.
3. Add a receipt-validation endpoint (`POST /api/v1/billing/store-receipt`)
   that verifies the App Store / Play receipt server-side and calls the same
   `upsert_subscription` the Stripe webhook uses. Tier logic, gating, and the
   entitlement checks need no changes.
4. Hide the Stripe checkout buttons when running inside a shell
   (`window.Capacitor` is defined) and show the native purchase sheet instead.
5. **Donations are web-only.** The app already hides the donate card inside
   store shells (`IN_STORE_SHELL` gate) — keep it that way: Apple treats
   in-app donations to the developer as digital purchases (3.1.1), and Play's
   payment policy is equivalent.

## Store assets

Listing copy, privacy labels, and review notes live in `../store/`.
