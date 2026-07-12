# lifeline-native (Capacitor plugin)

One plugin that backs every `window.Lifeline*` bridge the web app calls. The web
glue in `web/assets/native-bridge.js` adapts these methods into those bridges,
so once this plugin is linked, the app's IAP, notifications, on-device AI,
health read, native sign-in, device scan, and App Attest **light up
automatically** — no changes to the deployed web app.

## What's implemented vs. an integration point

| Method | iOS | Android |
|---|---|---|
| `purchase` (IAP) | ✅ StoreKit 2 | ✅ Play Billing 7 |
| `requestNotificationPermission` / `scheduleDaily` / `showNotification` | ✅ UserNotifications | ✅ NotificationCompat (wire AlarmManager for repeat) |
| `deviceProfile` | ✅ | ✅ |
| `signInApple` | ✅ ASAuthorization | n/a (iOS only) |
| `signInGoogle` | ⚙️ add GoogleSignIn pod | ⚙️ add Web client ID (Credential Manager) |
| `requestHealthPermission` / `readHealth` | ✅ HealthKit read | ⚙️ Health Connect query |
| `aiDownload` / `aiGenerate` | ⚙️ MediaPipeTasksGenAI | ⚙️ tasks-genai |
| `attest` (App Attest) | ✅ DeviceCheck | n/a (use Play Integrity) |

✅ = written against current APIs, open in Xcode / Android Studio to build.
⚙️ = a documented integration point with the exact SDK call noted inline — add
the (optional) SDK dependency and drop in the ~10 lines described in the code
comment. Every ⚙️ method rejects cleanly, so the web app just falls back (e.g.
to the cloud coach, or the simulated sign-in in dev) until you wire it.

## Build

```bash
cd native/plugins/lifeline-native
npm install && npm run build      # compiles the TS bridge
# From native/: `npx cap sync` picks the plugin up automatically (it's a local
# dependency in native/package.json).
```

## Capabilities to enable (Xcode) / permissions (Android)

- **iOS**: In-App Purchase, HealthKit, App Attest, Sign in with Apple, Push.
  `Info.plist`: `NSHealthShareUsageDescription`, `NSUserNotificationsUsageDescription`.
- **Android**: the plugin's `AndroidManifest.xml` declares POST_NOTIFICATIONS,
  the Health Connect read permissions, and BILLING.

Product IDs must match the backend: `health.lifeline.app.pro_monthly` and
`health.lifeline.app.elite_monthly`.
