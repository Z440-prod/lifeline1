# Privacy label answers

Answers derived from the actual architecture (see `web/privacy.html` and the
zero-knowledge design docs). Raw health data never reaches the server; vault
contents are ciphertext; arena data is pseudonymous. An **account** (email, or
Apple/Google sign-in) sits on top for continuity and recovery — it holds no
keys and no health data.

## Apple — App Privacy ("nutrition label")

| Question | Answer | Why |
|---|---|---|
| Health & Fitness data | **Not collected** | Read on-device only; never transmitted. The opaque vitality integer is derived, optional, and not identifiable health data — declare as *Usage Data → Product Interaction, not linked to identity* if reviewer requests. |
| Contact info — **email address** | **Collected, linked to identity, NOT used for tracking** | Email/password or Apple/Google sign-in identifies the account (authentication + recovery only). Apple Private Relay is honored when the user hides their email. Deletable in-app. |
| Identifiers | **Device ID — collected, not linked to tracking** | Random UUID + attestation public key; no IDFA. |
| User content | **Other user content — collected, encrypted such that provider cannot read** | E2EE vault ciphertext. |
| Purchases | **Purchase history — collected** | Subscription tier + status. |
| Usage data / diagnostics | Aggregated metrics only (Prometheus counters), contains no identifiers | |
| Tracking (ATT) | **No tracking.** No ATT prompt needed. | |

**Sign in with Apple (Guideline 4.8).** Because the app offers Google sign-in,
it also offers Sign in with Apple, which collects only name + email and supports
Apple's Private Relay. Email/password is provided as an equivalent option.

**Account deletion (Guideline 5.1.1(v)).** Settings → *Delete account* calls
`DELETE /api/v1/account`, which erases the account and all associated
data (vault, scores, subscription, device records) in one transaction, then
wipes local storage. No support ticket required.

## Google Play — Data safety

| Section | Answer |
|---|---|
| Data collected: Health info | **No** (processed on-device; never leaves it) |
| Data collected: Personal info (email) | Yes — for account sign-in/recovery; not shared; deletable in-app |
| Data collected: App activity | Yes — pseudonymous arena score/handle; optional |
| Data collected: Device or other IDs | Yes — random app-scoped device ID (not a resettable ad ID) |
| Data collected: Financial info | No (payments handled by Google Play / Stripe; no card data) |
| Data shared with third parties | **None** |
| Data encrypted in transit | Yes (TLS) |
| Data encrypted at rest | Yes (vault: client-side E2EE; tokens: ChaCha20-Poly1305) |
| Deletion mechanism | **In-app: Settings → Delete account** (`DELETE /api/v1/account`) erases the account and all associated server data |
| Independent security review | Optional — architecture supports it |

## One-line stance for both reviews

> Lifeline is zero-knowledge by construction: health data is processed
> exclusively on-device; servers store only client-side-encrypted blobs, a
> pseudonymous game score, subscription state, and an account email for sign-in.
> Everything is deletable in-app.
