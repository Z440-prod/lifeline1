# Lifeline — go-live runbook

This is the single path from "it runs on my laptop" to "it's live on my own
Stripe + my own database, with binaries uploaded to the App Store and Google
Play." Follow it top to bottom. Anything that costs money or touches a live
account is **yours to press** — this repo gives you the exact commands and files.

> **What's automated vs. what's yours.** The backend, database schema, and web
> app are fully handled by code here — one container, migrations self-apply on
> boot. The three things no repo can do for you: (1) enter your own secret keys,
> (2) build the iOS binary (needs a Mac + Xcode — Apple's rule, not ours), and
> (3) click "submit" in the two store consoles.

---

## The shape of it

```
                 ┌─────────────────────────────────────┐
   App Store  ───┤  iOS shell  ┐                        │
                 │             ├─►  https://app.lifeline.health   (your host)
   Google Play───┤  Android    ┘        │                │
                 └────────────────────── │ ───────────────┘
                                         ▼
                            Antigravity engine (Rust)
                              • serves the web app
                              • your Stripe (billing)
                              • your Postgres/Supabase (data)
                              • your Anthropic key (coach)
```

The native apps are **thin shells** that load your deployed web app
(`native/capacitor.config.json` → `server.url`). So you deploy the engine once,
and both stores show the same always-current app. Ship a web update → both apps
update instantly, no re-review (for web-layer changes).

---

## Step 1 — Deploy the engine (your host)

The engine is a persistent Rust server with a live Postgres connection, so it
needs a real host — **not** Vercel or Cloudflare serverless. Recommended: **Fly.io**
(managed Postgres + free TLS). Render/Railway/a VM work identically via the
Dockerfile.

```bash
cp deploy/.env.production.example deploy/.env.production   # then fill it in
openssl rand -hex 48        # paste as ANTIGRAVITY__AUTH__SERVER_SECRET

# Fly.io:
fly launch --no-deploy --copy-config --dockerfile deploy/Dockerfile
fly secrets import < deploy/.env.production
fly deploy
```

On boot the engine connects to your Postgres and **auto-applies all migrations**
(`sqlx::migrate!`) — no manual DB step. In `production` mode it will **refuse to
start** on a placeholder secret, a missing Anthropic key, or an unreachable DB.
That's the safety net: if it's up, it's configured correctly.

Point your domain (`app.lifeline.health`) at the host and confirm:
`curl https://app.lifeline.health/health` → `{"status":"ok"}`.

## Step 2 — Your database (Supabase)

Supabase **is** Postgres, so it just works as the `DATABASE__URL`:
1. Supabase dashboard → Project Settings → Database → Connection string (URI).
2. Paste it into `ANTIGRAVITY__DATABASE__URL` in `deploy/.env.production`.
3. That's it — the engine creates every table on first boot.

> If you'd rather I run the migrations for you via the Supabase connector, enable
> the **Supabase** connector *in this chat* (it's connected to your org but toggled
> off here) and tell me the project — I can apply the schema and verify it.

## Step 3 — Your Stripe (billing)

Create the two subscription products in **your** Stripe, then wire the IDs in.

1. dashboard.stripe.com → Product catalog → add:
   - **Lifeline Pro** — recurring, e.g. $7.99/mo → copy its **Price ID** →
     `ANTIGRAVITY__BILLING__PRICE_PRO`
   - **Lifeline Elite** — recurring, e.g. $14.99/mo → copy its **Price ID** →
     `ANTIGRAVITY__BILLING__PRICE_ELITE`
2. Developers → API keys → copy the **live secret key** →
   `ANTIGRAVITY__BILLING__STRIPE_SECRET_KEY`.
3. Developers → Webhooks → add endpoint
   `https://app.lifeline.health/billing/webhook`, subscribe to
   `checkout.session.completed`, `customer.subscription.updated`,
   `customer.subscription.deleted` → copy the **signing secret** →
   `ANTIGRAVITY__BILLING__STRIPE_WEBHOOK_SECRET`.
4. Redeploy so the new secrets load.

> If you'd rather I create the products/prices in your account automatically,
> enable the **Stripe** connector *in this chat* (also connected-but-off here). I
> can create the two products, pull the Price IDs, and hand you the exact env
> lines. I will **not** create live charges or a webhook that moves money without
> confirming the amounts and account with you first.

## Step 4 — Build the store binaries

The native project lives in `native/`. It wraps your deployed web app.

```bash
cd native
# Point the shell at your live engine (edit capacitor.config.json → server.url)
npm install
npm run assets            # generates icons + splash from native/assets/icon.svg
```

### Android → `.aab` for Google Play  (buildable on Linux/Mac/Windows)
```bash
npm run add:android
npx cap sync android
cd android && ./gradlew bundleRelease
# → android/app/build/outputs/bundle/release/app-release.aab
```
Sign it with **your** upload keystore (Play Console → create one, or let Play
manage signing). Requires the Android SDK installed locally.

### iOS → `.ipa` for the App Store  (requires a Mac + Xcode — Apple's rule)
```bash
npm run add:ios
npx cap sync ios
npm run open:ios          # opens Xcode → Product → Archive → Distribute
```
This step **cannot be done on this Linux server or any non-Apple machine** — it's
an Apple platform requirement, not a limitation of this project. Everything up to
the Xcode archive is prepared for you.

## Step 5 — Submit to the stores

Listing copy, privacy labels, and review notes are already written:
- `store/LISTING.md` — title, subtitle, description, keywords
- `store/PRIVACY_LABELS.md` — App Privacy answers (zero-knowledge)
- `store/REVIEW_NOTES.md` — a demo account + the account-deletion path (5.1.1(v))
- `store/LAUNCH_CHECKLIST.md` — the full pre-submit list

Upload the `.aab` to Google Play Console and the `.ipa` (via Xcode/Transporter)
to App Store Connect, paste the listing, and submit.

---

## What each cost is, so there are no surprises

| Line item | Who charges you | Typical |
|---|---|---|
| Apple Developer Program | Apple | $99 / year |
| Google Play Developer | Google | $25 one-time |
| Host (Fly/Render/Railway) | the host | ~$5–20 / mo to start |
| Postgres (Supabase free tier ok to start) | Supabase | $0 → up |
| AI coach — cloud tier (non-premium devices) | open-source host (Together/Groq/OpenRouter) or Anthropic | pay-per-use (open-source is cheaper) |
| Stripe | Stripe | ~2.9% + 30¢ / charge |

The **app code** is done and verified. These are the only things standing between
here and live — and every one of them is a key you own or a button in a console
that has to be you.

---

## If you want me to do more from here

Three connectors are attached to your org but **toggled off in this chat**
(Stripe, Supabase, Vercel). If you enable any of them in this conversation's
connector settings, I can go further:
- **Supabase on** → I apply the schema to your project and verify the tables.
- **Stripe on** → I create the two products and hand back the Price IDs (no live
  charges without your explicit OK).
- **Vercel on** → note: Vercel can host the *web* layer but **not** this Rust
  engine (it's a stateful server). For the engine, use Fly/Render/Railway above.

I still can't build the iOS binary (no Mac here) or press "submit" in the store
consoles for you — those are yours by design.
