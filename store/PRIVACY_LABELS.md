# Privacy label answers

Answers derived from the actual architecture (see `web/privacy.html` and the
zero-knowledge design docs). Raw health data never reaches the server; vault
contents are ciphertext; arena data is pseudonymous.

## Apple — App Privacy ("nutrition label")

| Question | Answer | Why |
|---|---|---|
| Health & Fitness data | **Collected, linked to user? NO — not collected** | Read on-device only; never transmitted. The opaque vitality integer is derived, optional, and not identifiable health data — declare as *Usage Data → Product Interaction, not linked to identity* if reviewer requests. |
| Contact info (name, e-mail, phone) | Not collected | There are no accounts. |
| Identifiers | **Device ID — collected, not linked to identity, not used for tracking** | Random UUID + attestation public key; no IDFA. |
| User content | **Other user content — collected, not linked, encrypted such that provider cannot read** | E2EE vault ciphertext. |
| Purchases | **Purchase history — collected, not linked** | Subscription tier + status. |
| Usage data / diagnostics | Aggregated metrics only (Prometheus counters), contains no identifiers | |
| Tracking (ATT) | **No tracking.** No ATT prompt needed. | |

## Google Play — Data safety

| Section | Answer |
|---|---|
| Data collected: Health info | **No** (processed on-device; never leaves it) |
| Data collected: App activity | Yes — pseudonymous arena score/handle; optional; user can request deletion (identity reset) |
| Data collected: Device or other IDs | Yes — random app-scoped device ID (not resettable ad ID) |
| Data collected: Financial info | No (payments handled by Google Play / Stripe; no card data) |
| Data shared with third parties | **None** |
| Data encrypted in transit | Yes (TLS) |
| Data encrypted at rest | Yes (vault: client-side E2EE; tokens: ChaCha20-Poly1305) |
| Deletion mechanism | In-app identity reset severs all server-side data |
| Independent security review | Optional — architecture supports it |

## One-line stance for both reviews

> Lifeline is zero-knowledge by construction: health data is processed
> exclusively on-device; servers store only client-side-encrypted blobs, a
> pseudonymous game score, and subscription state.
