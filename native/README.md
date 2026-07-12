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
- **Sign in with Apple**: enable the capability (required by Guideline 4.8
  because the app also offers Google sign-in). The web layer already renders the
  Apple button; wire it to `ASAuthorizationAppleIDProvider` and pass the id-token
  to `POST /account/oauth`.

## Account deletion (Guideline 5.1.1(v))

Already implemented and shipped in the web layer: **Settings → Delete account**
calls `DELETE /api/v1/account`, which erases the account and all associated data
server-side (encrypted vault, scores, subscription, device records) and wipes
local storage. No native work required beyond surfacing Settings.

## On-device AI (optional, premium devices)

The coach can run a local model (Gemma) so it works with no network. The web
layer detects capability (`web/assets/device.js`), offers the download, and
routes replies through a bridge the shell injects. Implement it with the OS ML
runtime (MediaPipe LLM Inference on Android, Core ML / MediaPipe on iOS):

```js
window.LifelineLocalAI = {
  isReady: () => boolean,                       // is a model installed & loaded
  download: async (modelId, onProgress) => {},  // fetch weights, report 0–100; verify sha256
  generate: async (prompt, { system, context, maxTokens }) => 'reply text',
  remove: async () => {},                       // delete weights, free space
};
// Optional richer device facts for the scanner:
window.LifelineDevice = { profile: { ram_gb, cores, chipset, os, os_version, has_npu, ai_backends: ['native-mediapipe'] } };
```

The model catalog (sizes, hardware floors) is served rules-only at
`GET /api/v1/ai/local-models`. **Gate large downloads to Wi-Fi** and confirm the
size with the user (the web card already displays it). The downloaded weights
are *data, not executable code* (Guideline 2.5.2).

## ⚠️ Subscriptions in the store builds

Apple (App Review 3.1.1) and Google (Play Payments policy) both require
**in-app digital subscriptions to use their own billing** — Stripe Checkout is
for the web app only and must not be reachable as a purchase path inside the
store binaries.

The backend is already shaped for this: entitlements hang off a `tier` per
device, and Stripe is just one writer of that state. Launch plan:

1. Create matching subscription products in App Store Connect and Play Console
   (`health.lifeline.app.pro_monthly` $7.99,
   `health.lifeline.app.elite_monthly` $14.99).
2. In the shells, purchase via StoreKit 2 / Play Billing, then expose the
   result to the web layer as the **`window.LifelineIAP` bridge**:

   ```js
   window.LifelineIAP = {
     // Runs the native purchase sheet for "pro" | "elite" and resolves with
     // the proof the backend verifies. Reject on cancel.
     purchase: async (tier) => ({
       platform: 'apple',            // or 'google'
       receipt: '<base64 App Store receipt | Play purchase token>',
     }),
   };
   ```

   The web app already does the rest: inside a shell the Plans page shows
   **Subscribe** buttons that call this bridge and redeem the result at
   `POST /api/v1/billing/store-receipt` (implemented). The endpoint verifies
   Apple receipts server-side via `verifyReceipt` (set
   `ANTIGRAVITY__BILLING__APPLE_SHARED_SECRET`; sandbox receipts retry
   automatically) and feeds the same `upsert_subscription` the Stripe webhook
   uses — gating is identical everywhere. Google Play verification requires a
   Play Developer API service account; until configured the endpoint refuses
   rather than trusting the client.
3. Stripe surfaces are already hidden inside shells (`IN_STORE_SHELL` gate):
   upgrade buttons become native Subscribe buttons, the billing portal is
   replaced by "manage in your store settings", and the donate card is
   removed entirely.
4. **Donations are web-only.** Apple treats in-app donations to the developer
   as digital purchases (3.1.1), and Play's payment policy is equivalent.

## Store assets

Listing copy, privacy labels, and review notes live in `../store/`.
