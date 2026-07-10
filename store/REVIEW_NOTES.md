# Notes for app review (Apple / Google)

## How to test without an account
Lifeline has **no accounts** by design. Identity is a hardware-attested device
key (Apple App Attest on iOS). Launch the app and it provisions itself —
onboarding → the Today portrait appears immediately. Nothing to sign into.

## Subscriptions
- Store builds purchase through StoreKit / Play Billing (products
  `pro_monthly` $7.99, `elite_monthly` $14.99).
- The free tier is fully functional (daily portrait, one connected source,
  leaderboard viewing). Paid tiers unlock competing in weekly seasons, fusing
  all sources, biomarker tracking, unlimited history and coaching, and (Elite)
  beta access. All entitlements are enforced server-side.

## Health data
All health signals are processed **on the device**. The server publishes rule
tables (reference ranges, model coefficients) and receives either
client-encrypted blobs it cannot decrypt or a single opaque 0–100 score the
user explicitly logs to the leaderboard. See the privacy policy at `/privacy`.

## AI coach
Messages route through a privacy proxy that strips identity and metadata; no
conversation is stored or used for training. The coach declines medical
diagnosis and directs users to clinicians where appropriate (clinical-first
policy matrix, versioned, served at `/api/v1/ai/policy-matrix`).

## Leaderboard content safety
Handles are restricted to `[A-Za-z0-9_]{3,20}` (no spaces, no Unicode) which
prevents impersonation/PII in names; profiles are pseudonymous and carry no
user content beyond the handle.
