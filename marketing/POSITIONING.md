# Lifeline — competitive positioning & "us vs. them" playbook

The market is full of health apps that treat your body as **their** data. Lifeline's
entire wedge is the opposite: your health is computed and encrypted **on your
device**, and you compete on it globally without ever handing it over. This doc
turns that into a campaign.

> ⚖️ **Truthfulness guardrail (read first).** Comparative advertising is legal
> and effective **when it's accurate.** Everything below leans on (a) Lifeline's
> own verifiable design and (b) each competitor's *publicly described* model.
> Before publishing any specific number (a price, a "sells your data" claim, a
> breach), **verify it against the competitor's current public materials.** Keep
> the contrast on architecture and business model — that's where we win and
> where the claims are defensible. Don't invent breaches, quotes, or data-sale
> accusations.

---

## The one-line position

**Lifeline is the zero-knowledge alternative to cloud health apps: your data
never leaves your device, there's no band or ring to buy, the AI runs on your
phone, and you compete on your health — not on your data.**

## The villain (the category we're the alternative to)

The default model of the health/longevity category:
1. **They store your body in their cloud.** Your sleep, HRV, and labs live on
   their servers.
2. **They rent you hardware.** A band or ring plus a monthly subscription just
   to see your own numbers.
3. **Their AI is their cloud.** Your context is sent off-device to answer you.
4. **Some monetize you.** Ads and data are how "free" apps stay free.
5. **Black boxes.** A "readiness score" you can't inspect.

Lifeline inverts all five. That's the whole story.

---

## Messaging pillars (hammer these)

| # | Pillar | The line |
|---|---|---|
| 1 | **Zero-knowledge** | "They watch your body. We literally can't." Data is computed on-device; the server only ever holds ciphertext it can't read. |
| 2 | **Bring your own device** | "No $300 ring. No $30/month band. Use the wearable you already own." |
| 3 | **On-device AI** | "A coach that runs on your phone — offline, private, instant. Your health never leaves the device to get an answer." |
| 4 | **Compete on health** | "The global Arena: rank on your vitality, not your follower count. Rivals see a number — never a biometric." |
| 5 | **No ads, no tracking, no data sales** | "You're not the product. There's nothing to sell — by design we can't read your data." |
| 6 | **Transparent by design** | "Your Lifeline Age is an open model you can inspect — not a black box." |
| 7 | **Erasable in one tap** | "Delete your account and everything tied to it, instantly. GDPR-clean." |

## Taglines / hooks

- **Your health. Zero-knowledge.**
- **They watch your body. We never do.**
- **Compete on your health — not your data.**
- **No band. No ring. No cloud. No catch.**
- **The longevity app that forgets you on purpose.**
- **Your body isn't their business.**
- **Bring your own wearable. Keep your own data.**

---

## Per-competitor "switch" angles

Each is framed around a **publicly-known model difference**, not a smear. Verify
prices before use — they change.

**vs. Whoop** (subscription band, recovery/strain in their cloud)
> "Whoop sends your recovery to their servers — and charges you a yearly
> membership for the band. Lifeline reads the wearable you *already* own, scores
> your readiness **on your phone**, and never sees your data. Same insights.
> Your hardware. Your data."

**vs. Oura** (ring + subscription, readiness/sleep in their app)
> "A $300+ ring and a monthly fee to read your own sleep — stored in their
> cloud. Lifeline fuses sleep and readiness on-device from any source you
> already have. No ring required."

**vs. Apple Health / Fitness+** (great sensors, one ecosystem, no game)
> "Apple's sensors are excellent — and locked to Apple. Lifeline fuses Apple
> Health, Google Health Connect, and Whoop into **one** readiness score, adds a
> transparent biological age, and lets you compete globally. Cross-platform,
> and still on-device."

**vs. Noom / MyFitnessPal** (free, ad- and data-supported behavior/nutrition)
> "If it's free, you're the product — ads and data. Lifeline has no ads, no
> trackers, and **can't** read your data by construction. Health, not
> harvesting."

**vs. InsideTracker / Function Health** (biomarker longevity in their cloud)
> "Expensive panels, uploaded to their servers. Lifeline plots your labs against
> healthy reference ranges **on your device** — the values never leave it. Bring
> the bloodwork you already have."

**vs. Strava** (social/competitive fitness, public activity feeds)
> "Strava turns your movements into a public feed. Lifeline's Arena lets you
> compete on your *health* with a single opaque score — a leaderboard that
> ranks you blind. Competition without surveillance."

**vs. Blueprint / longevity-influencer stacks** (expensive, closed protocols)
> "You don't need a celebrity's budget or a black-box protocol. Lifeline gives
> you a transparent biological-age model, a private coach, and a daily rhythm
> that adapts to *you* — for the price of an app."

---

## Ad / social copy (short-form)

- "Your Oura ring knows your sleep. So does Oura. With Lifeline, only *you* do." *(verify framing)*
- "Delete your health app. Keep your health. → Lifeline."
- "Every health app: 'trust us with your data.' Lifeline: 'we designed it so you don't have to.'"
- "Cancel the band subscription. Your phone already has a coach — and it runs offline."
- "A leaderboard for your health that literally can't see your health."
- "Zero-knowledge isn't a privacy policy. It's the architecture."

## Landing surfaces

- **/compare** — the "Why switch to Lifeline" page (in this repo, `web/compare.html`).
- App Store subtitle: *"Your health. Zero-knowledge."* (already in `store/LISTING.md`).
- Paid social: lead with pillar 1 (zero-knowledge) + pillar 2 (no hardware fee);
  they're the sharpest wedges against Whoop/Oura.

## Proof points (why our claims are credible)

These are real and demonstrable — cite them, they're our moat:
- End-to-end encrypted vault (AES-256-GCM); the server stores ciphertext it
  cannot decrypt.
- On-device insights engine + optional on-device **Gemma** model (works offline).
- Only an opaque 0–100 score is ever shared, and only if you log it.
- In-app account deletion wipes everything server-side.
- No third-party analytics SDKs; no ad identifiers.

> Keep the tone confident, not preachy. We don't need to trash anyone — the
> architecture does the talking. "They can't help storing your data. We can't
> help *not* storing it."
