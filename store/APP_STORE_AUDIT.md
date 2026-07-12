# Lifeline — Pre-Submission App Store Audit

Run with the `appstore-readiness` skill (Reviewer · Privacy · Commerce · Designer
· Technical · Metadata agents). Grounded in the actual codebase, not assumptions.
Guidelines change — verify against current Apple docs before submitting.

```
┌─────────────────────────────────────────────────────────────┐
│                 PRE-SUBMISSION AUDIT REPORT                   │
│  App: Lifeline (health.lifeline.app)   Date: 2026-07-12       │
│  Overall Risk: 🟡 MEDIUM — one blocking gap now fixed;         │
│                remaining items are native-project config       │
├─────────────────────────────────────────────────────────────┤
│  BLOCKING (Must Fix)                                          │
│   1. Privacy manifest missing → FIXED (added at              │
│      native/privacy/PrivacyInfo.xcprivacy). Guideline 5.1.1   │
│   2. HealthKit Info.plist usage strings — must be added when  │
│      the iOS project is scaffolded. Guideline 5.1.1 / 5.1.3   │
├─────────────────────────────────────────────────────────────┤
│  WARNINGS (Should Fix)                                        │
│   3. Health/medical disclaimer must be visible in-app. 1.4.1  │
│   4. Reviewer needs the live server URL + demo account. 2.1   │
│   5. Confirm privacy labels match the manifest. 5.1.1(i)      │
├─────────────────────────────────────────────────────────────┤
│  CLEAR (verified in code)                                     │
│   ✓ IAP uses StoreKit; no web-payment surfaces in shell 3.1.1 │
│   ✓ Account deletion in-app (DELETE /account)     5.1.1(v)    │
│   ✓ Sign in with Apple offered                    4.8         │
│   ✓ No tracking / no ad SDKs → no ATT prompt      5.1.2       │
└─────────────────────────────────────────────────────────────┘
```

---

## 🔴 BLOCKING

### 1. Privacy manifest — FIXED
**Guideline 5.1.1 + the May-2024 privacy-manifest rule.** No
`PrivacyInfo.xcprivacy` existed anywhere in `native/`. Apps are rejected without
one. **Added** `native/privacy/PrivacyInfo.xcprivacy` (validated plist): declares
`NSPrivacyTracking=false`, no tracking domains, collected types (Email, Health,
Fitness, UserID, PurchaseHistory — all *linked, app-functionality, not tracking*),
and the UserDefaults required-reason API (`CA92.1`, used by Capacitor).
- **Action:** after `npx cap add ios`, copy it to
  `native/ios/App/App/PrivacyInfo.xcprivacy` and add to the App target.
- **Also:** each bundled third-party SDK must ship its own manifest; verify the
  Capacitor plugins you use are on Apple's manifest-required list and updated.

### 2. HealthKit Info.plist usage strings
**Guideline 5.1.1 (purpose strings) + 5.1.3 (health data).** HealthKit reads
crash on launch without a purpose string, and review rejects missing/again-vague
strings. When the iOS project is generated, add to `Info.plist`:
```xml
<key>NSHealthShareUsageDescription</key>
<string>Lifeline reads your health metrics on-device to compute your private
vitality score. Your health data never leaves your phone unencrypted.</string>
<key>NSUserNotificationsUsageDescription</key>
<string>Lifeline sends one optional daily check-in with your private anecdote.</string>
```
Enable the **HealthKit**, **In-App Purchase**, **App Attest**, **Sign in with
Apple**, and **Push** capabilities in Xcode. HealthKit rules (5.1.3): never use
health data for advertising/marketing, never store it in iCloud, only share for
health purposes — Lifeline's zero-knowledge design already satisfies this.

---

## 🟡 WARNINGS

### 3. Visible medical disclaimer — **Guideline 1.4.1 / 5.1.3**
Lifeline surfaces a "Lifeline Age" / biological-age and longevity guidance. The
review notes mention a clinical-first tone, but Apple wants a **user-visible**
statement that the app is not a medical device and doesn't diagnose/treat. Add a
one-line disclaimer in Settings and near the Lifeline Age (e.g. *"Lifeline is for
informational and wellness purposes and is not a medical device; it doesn't
diagnose or treat any condition. Consult a clinician for medical decisions."*).

### 4. Reviewer access — **Guideline 2.1**
The native shell loads `https://app.lifeline.health`, so **the server must be
live and reachable during review** or the app shows a blank/errored webview →
rejection. In App Store Connect → App Review Information: provide a working demo
account (already drafted in `store/REVIEW_NOTES.md`) and confirm the production
engine is deployed (`deploy/GO-LIVE.md`) before you submit.

### 5. Labels ↔ manifest ↔ reality must match — **Guideline 5.1.1(i)**
`store/PRIVACY_LABELS.md` must exactly match the new manifest and the App Store
Connect nutrition labels. Decide and keep consistent: Health & Fitness is
declared **collected/linked** here (because encrypted health blobs transmit
off-device even though the server can't read them). If you instead claim "Data
Not Collected" for health on the strength of E2EE, reviewers who see uploads may
push back — the conservative declaration above is the safer path. Pair it with
the privacy policy (`web/privacy.html`) explaining the zero-knowledge model.

---

## ✅ CLEAR — verified in the code

- **3.1.1 In-App Purchase.** `web/assets/app.js` gates on `IN_STORE_SHELL`
  (`window.Capacitor` present): in the store build only the StoreKit `data-iap`
  "Subscribe" button renders; the Stripe upgrade button, billing-portal link,
  and donation card are all suppressed, with copy stating "Purchases are handled
  by the App Store / Google Play." No external purchase surfaces in the app — the
  #1 monetization rejection is avoided. Receipts verify server-side via
  `storeReceipt`.
- **5.1.1(v) Account deletion.** `DELETE /account` + in-app "Delete Account" flow
  wipes account and all linked data. Compliant.
- **4.8 Sign in with Apple.** Offered alongside Google and email (the plugin
  implements it), satisfying the equivalent-login requirement.
- **5.1.2 / ATT.** No third-party ad SDKs, no cross-app tracking, no IDFA →
  **no ATT prompt required.** `NSPrivacyTracking=false` is honest. This is a
  genuine strength for a health app.
- **4.2 Minimum functionality.** Not a thin web wrapper: real native capabilities
  (HealthKit, App Attest, StoreKit, notifications, on-device AI) via the
  LifelineNative plugin. Lead the reviewer through these in the notes so the
  remote-URL architecture reads as a native app, not a website.

---

## Metadata / ASO quick pass
- **Name** "Lifeline" ≤ 30 chars ✓ (verify no trademark conflict in your region).
- **Screenshots** delivered at 1290×2796 (6.7"/6.9") in `store/screenshots/` ✓ —
  show the app in use, not splash/login ✓.
- Description must include ToS + Privacy links and avoid competitor names /
  unverifiable medical claims. Age rating: answer the health questionnaire
  honestly (no medical-diagnosis claims).

## Launch gate status
Blocking items: #1 fixed in-repo; #2 is a scaffold-time step (documented). Once
the iOS project is generated with the manifest + Info.plist strings, the disclaimer
is added, and the server is live with a demo account, Lifeline clears the hard gate.

---

## Re-check — 2026-07-12 (after the device-adaptive AI coach)

The coach now picks its engine by device: premium phones run Gemma on-device;
everyone else uses a cheaper open-source model (Llama/Qwen/DeepSeek via an
OpenAI-compatible endpoint) or Claude, chosen server-side by `[ai] provider`.
Re-audited against the skill — **no new risk introduced:**

- **5.1.1 / 5.1.2 privacy — unchanged.** The open-source model is a *server-side*
  backend the client never sees; it adds no SDK, no client identifier, and no new
  data type. The proxy still strips identity before any model sees a word. No new
  `NSPrivacyCollectedDataType` and no tracking → the manifest is unaffected, no
  ATT prompt. ✅
- **2.1 functionality — improved.** With a provider key set the coach returns real
  answers (verified end-to-end against a mock open-source endpoint: request →
  normalize → render). This strengthens the "app is fully functional in review"
  requirement vs. the old dev-mock echo. ✅
- **3.1.1 / 5.1.1(v) / 4.8 — still CLEAR** (unchanged by this work).
- **Cost/abuse — unchanged.** The three-gate token budget still meters every coach
  call regardless of provider. ✅

**Verdict unchanged: 🟡 MEDIUM**, gated only on the same native-scaffold items
(#2) plus the label/manifest consistency check (#5). The adaptive coach does not
move the risk.
