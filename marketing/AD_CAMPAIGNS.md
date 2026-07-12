# Lifeline — ready-to-run ad campaign kit

Everything here is **paste-ready**. It's built on the merged positioning
(`marketing/POSITIONING.md`) and the `/compare` landing page. Character counts
are pre-fit to each platform's limits as of writing — re-check the platform's
current limits before you paste, they drift.

> ⚖️ **Same truthfulness rule as POSITIONING.md.** Every line here leans on
> Lifeline's own verifiable design or the *category's* publicly-described model.
> Before you spend a cent on any ad that names a competitor or cites a price,
> verify the specific claim against that company's **current** public materials.
> Keep the contrast on architecture and business model — that's defensible.
>
> 🚦 **Nothing here has been launched.** No live campaign exists. Running these
> spends real money — set the account, budget, targeting, and approve creatives
> before you push "publish."

---

## 0. Campaign map (what to run where)

| Platform | Objective | Why | Start budget |
|---|---|---|---|
| **Apple Search Ads** | App installs | Highest-intent: people searching "Oura app", "Whoop", "longevity" on the App Store | $20–40/day |
| **Google Ads (Search)** | App installs / web to `/compare` | Capture "alternative to…" and privacy-health intent | $20–40/day |
| **Meta (IG/FB)** | App installs | Best for the privacy + "no hardware fee" emotional hooks | $15–30/day |
| **TikTok** | App installs / traffic | Short demo of on-device AI + Arena; younger longevity-curious | $20/day |
| **Reddit** | Traffic to `/compare` | r/QuantifiedSelf, r/Biohackers, r/privacy skew exactly to our wedge | $10–20/day |

**One rule that saves money:** start each platform with **one** campaign, **2–3
ad sets** (one per pillar), **3 creatives each**. Kill the bottom third every 3
days. Don't scale until CPI (cost per install) is stable for 5+ days.

---

## 1. Apple Search Ads (highest priority)

People searching the App Store for health/longevity apps are already sold on the
category — you just have to be the privacy-first option in the results.

**Keyword themes (exact + broad):**
- Competitor terms: `oura app`, `whoop`, `readiness score`, `inside tracker`, `biological age`
- Category terms: `longevity`, `healthspan`, `hrv`, `sleep score`, `recovery tracker`
- Wedge terms: `private health app`, `offline health`, `encrypted health`, `no subscription health`

**Custom Product Pages (each maps to a pillar — point the ad at the matching one):**

*CPP A — Privacy wedge*
- Headline: `Your health. Zero-knowledge.`
- Subhead: `We literally can't read your data.`

*CPP B — No hardware fee*
- Headline: `No ring. No band. No fee.`
- Subhead: `Use the wearable you already own.`

*CPP C — Compete*
- Headline: `Rank on your health.`
- Subhead: `A leaderboard that can't see your body.`

> Apple pulls ad text from your listing metadata, so the real lever here is the
> **App Store listing + CPPs** in `store/LISTING.md`. Keep those pillar-aligned.

---

## 2. Google Ads — Search

**Campaign type:** App campaign (for installs) *or* Search → `/compare` (for web).
For Search-to-web, here are the assets. Responsive Search Ad — give Google 15
headlines + 4 descriptions and let it assemble.

**Headlines (≤30 chars each):**
1. `Health App That Can't See You`
2. `Zero-Knowledge Health App`
3. `No Ring. No Band. No Fee.`
4. `Your Body Isn't Their Data`
5. `Offline AI Health Coach`
6. `The Private Oura Alternative` *(verify before naming Oura)*
7. `Track Health, Keep It Private`
8. `Biological Age, On Your Phone`
9. `Compete On Your Health`
10. `Encrypted. Offline. Yours.`
11. `Cancel The Band Subscription`
12. `A Coach That Runs Offline`
13. `Longevity Without The Cloud`
14. `Bring Your Own Wearable`
15. `Health Data Never Leaves You`

**Descriptions (≤90 chars each):**
1. `Sleep, HRV & labs computed on your device. The server only holds data it can't read.`
2. `No $300 ring, no monthly band fee. Use the wearable you already own. Free to start.`
3. `On-device AI coach that works offline. Your health never leaves the phone for answers.`
4. `Rank globally on your vitality — rivals see a number, never a biometric. Try it free.`

**Keyword ideas (start exact/phrase, mine search terms weekly):**
- `alternative to oura`, `alternative to whoop`, `private health app`,
  `offline health tracker`, `encrypted health app`, `biological age app`,
  `longevity app`, `hrv app no subscription`

**Negative keywords:** `free ring`, `discount code`, `crack`, `jobs`, `stock`.

**Sitelinks:** `How the privacy works → /privacy` · `Why switch → /compare` ·
`On-device AI → /` · `Delete anytime → /privacy`

---

## 3. Meta (Instagram + Facebook)

Three ad sets, one per pillar. Single-image or 15s video. Primary text first,
headline is the bold line under the image.

**Ad set 1 — Zero-knowledge**
- Primary text: `Every health app says "trust us with your data." Lifeline is built so you don't have to. Your sleep, HRV, and labs are computed on your device and encrypted — the server only ever holds data it can't read. A longevity app that forgets you on purpose.`
- Headline: `They watch your body. We never do.`
- Description: `Zero-knowledge health. Free to start.`
- CTA: `Download`

**Ad set 2 — No hardware fee**
- Primary text: `A $300 ring. A $30/month band. Just to read your own sleep. Lifeline uses the wearable you already own — Apple Health, Google, or Whoop — and fuses it into one readiness score. No hardware. No lock-in.`
- Headline: `No ring. No band. No fee.`
- Description: `Bring your own wearable.`
- CTA: `Download`

**Ad set 3 — On-device AI**
- Primary text: `Your AI health coach shouldn't need to phone home. On supported phones, Lifeline runs the coach as a local model — offline, instant, and nothing leaves your device to get an answer.`
- Headline: `A coach that runs offline.`
- Description: `On-device AI. Truly private.`
- CTA: `Download`

**Audiences to test:** interest stacks on Oura / Whoop / QuantifiedSelf /
biohacking / data privacy; lookalike from installers once you have ~100.

---

## 4. TikTok / Reels / Shorts (organic + paid share the same scripts)

Vertical, 15–25s, captions burned in (most watch muted). Three hooks:

**Script A — the "can't see it" demo**
- 0–3s (hook, on screen): `every health app stores your body in their cloud.`
- 3–8s: screen-record opening Lifeline, score animating in. VO/caption: `this one computes it all on your phone.`
- 8–15s: toggle airplane mode, coach still answers. Caption: `no internet. still works. it's all on-device.`
- 15–20s (payoff): `a health app that literally can't see your health.`
- CTA card: `Lifeline — free on the App Store.`

**Script B — cancel the subscription**
- Hook: `POV: you just realized you're paying $30/month to read your own sleep.`
- Body: show Lifeline pulling from the wearable they already own → one score.
- Payoff: `no ring. no band. no fee. bring your own wearable.`

**Script C — the Arena**
- Hook: `a leaderboard for your health that can't see your health.`
- Body: show climbing a league, opponent shown only as an opaque score.
- Payoff: `compete on your vitality — not your data.`

---

## 5. Reddit (traffic → `/compare`)

Reddit hates ads that smell like ads. Lead with the architecture, not the pitch.
Target r/QuantifiedSelf, r/Biohackers, r/privacy, r/longevity.

- Title: `A health tracker where the server literally can't read your data (on-device + E2EE)`
- Body: `Built Lifeline because I didn't want my sleep/HRV/labs sitting on someone's server. Everything's computed on-device; the backend only stores ciphertext. On supported phones the AI coach runs locally so it works offline. Free to start, bring your own wearable. Breakdown of how the privacy actually works: [/compare]`
- CTA: `See how it works →`

---

## 6. App Store "Why switch" — long description block

Paste into the App Store description (or a CPP long text). Mirrors `store/LISTING.md`.

```
Most health apps store your body in their cloud — and rent you a band or ring to
do it. Lifeline is the opposite.

ZERO-KNOWLEDGE BY DESIGN
Your sleep, heart rate, and labs are computed on your device and end-to-end
encrypted. Our server only ever holds data it can't read. Not "we promise not to
look" — we built it so we can't.

BRING YOUR OWN WEARABLE
No $300 ring. No monthly band. Fuse Apple Health, Google Health Connect, and
Whoop into one readiness score using the hardware you already own.

AN AI COACH THAT RUNS OFFLINE
On supported phones, your coach runs a local model — private, instant, and it
works with no internet. Your health never leaves the device to get an answer.

COMPETE ON YOUR HEALTH
Climb the global Arena on your vitality. Rivals see a single opaque score —
never a biometric. A leaderboard that ranks you blind.

TRANSPARENT AND ERASABLE
An open biological-age model you can inspect, not a black box. Delete your
account and everything tied to it in one tap.

No ads. No trackers. No data sales — by design, we can't.
Your health. Zero-knowledge.
```

---

## 7. UTM + measurement (so you can actually read results)

Tag every web-destination ad so Supermetrics (analytics) can attribute it later:

```
?utm_source={platform}&utm_medium=cpc&utm_campaign={pillar}&utm_content={creative_id}
```

Example: `/compare?utm_source=google&utm_medium=cpc&utm_campaign=zero-knowledge&utm_content=rsa-h1`

- **App installs:** rely on the platform's own attribution (Apple Search Ads
  attribution, Meta/TikTok SDK) — UTMs don't survive the App Store bounce.
- **Web (`/compare`):** UTMs above; pull the report back through the Supermetrics
  connector once campaigns are live.

**North-star metrics:** CPI (cost per install) per platform, D1/D7 retention,
free→Pro conversion. Kill any ad set 30%+ above your blended CPI after 3 days.

---

## 8. Launch checklist (before you spend a cent)

- [ ] App Store listing + Custom Product Pages match the pillars (`store/LISTING.md`)
- [ ] `/compare` is live and loads fast on mobile
- [ ] Every competitor name / price in a live ad re-verified this week
- [ ] Conversion tracking installed (Apple Search Ads attribution, Meta/TikTok pixel/SDK)
- [ ] Daily budget caps set on every campaign (so a runaway ad can't drain the card)
- [ ] One platform first (recommend **Apple Search Ads**), prove CPI, then expand
- [ ] A human approved the final creatives
```
