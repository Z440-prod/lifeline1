# Notes for app review (Apple / Google)

## How to sign in
On first launch you'll see onboarding, then a sign-in / sign-up screen. Any of:
- **Email + password** — create an account with any email and an 8+ character
  password.
- **Sign in with Apple** or **Continue with Google** — one tap.

The account is only an identity layer: it unlocks access and enables recovery
across devices. Your health data is still computed and encrypted on the device;
the account holds no keys and no health data. On iOS the device itself is also
hardware-attested (Apple App Attest).

## Account deletion (Guideline 5.1.1(v))
**Settings → Delete account.** A confirmation sheet, then a permanent server-side
erase of the account and everything tied to it (encrypted vault, scores,
subscription, device records), plus a full local wipe. Implemented as
`DELETE /api/v1/account`. No email or support ticket required.

## Sign in with Apple (Guideline 4.8)
Because Google sign-in is offered, Sign in with Apple is offered alongside it,
collecting only name + email and honoring Apple's Private Relay.

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

## Daily notifications (opt-in)
Off by default. **Settings → Daily check-in** requests OS permission and then
sends one notification per day: a short, AI-written note about the user's day
(vitality, standout signal, rank, streak). The text is generated on-device — no
health data is transmitted; only the finished sentence reaches the notification
center. The app is fully functional with notifications off.

## On-device AI (optional, premium devices)
On capable phones, Settings offers to download a small on-device model (Gemma)
so the coach runs **entirely on the device** with no network. The downloaded
artifact is **model data, not executable code** — the app's logic is unchanged
(consistent with Guideline 2.5.2). The download size is shown before the user
opts in; the native shell restricts large downloads to Wi-Fi. Once installed,
the coach works fully offline (a service worker caches the app shell and the
public rule tables).

## Leaderboard content safety
Handles are restricted to `[A-Za-z0-9_]{3,20}` (no spaces, no Unicode) which
prevents impersonation/PII in names; profiles are pseudonymous and carry no
user content beyond the handle.
