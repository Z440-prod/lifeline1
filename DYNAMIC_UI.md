# Lifeline — the dynamic, per-user app (architecture + store compliance)

**The goal:** every user gets their own version of the app — UI, layout, which
features show, emphasis, theme, coach voice — shaped by what the app knows about
them and how they use it, with the AI arranging it.

**The key truth: you do not need a loophole.** An LLM literally writing and
running new app *code* per user is forbidden by both stores. But the thing you
actually want is achieved by a **sanctioned, mainstream architecture** —
server-driven / generative UI via a **declarative manifest** — that Airbnb, Lyft,
Spotify, and every feature-flag/A-B/remote-config system already ship on the App
Store. Lifeline is built this way.

---

## The architecture (and why it's compliant, not a loophole)

Three parts, and the boundary between them is what keeps it legal:

1. **A fixed catalog of blocks + a fixed renderer**, shipped inside the app
   binary and reviewed by Apple/Google like any other code. Examples:
   `readiness`, `age`, `circadian`, the feel slider, the surface card, each tab.
   Nothing here is downloaded.

2. **A composer that emits a MANIFEST** — plain data (JSON), from a *closed
   vocabulary of known block ids*. It says: which blocks, in what order, which to
   hide, which one to surface, what accent, what tab order. Today the composer is
   rules (`composer.js` + the Conductor + `usage.js` + `personalShape`). Tomorrow
   the **on-device LLM emits the same JSON shape** — that's "the AI arranges the
   app." See `web/assets/composer.js`.

3. **The renderer validates the manifest against an allowlist** and draws only
   known blocks, ignoring anything it doesn't recognize. So even an AI-authored
   manifest can only rearrange, show, or hide **pre-built, already-reviewed**
   blocks. It can never introduce new executable behavior or change the app's
   advertised purpose.

```
   what the AI sees            emits DATA (not code)          fixed, reviewed
 ┌────────────────────┐      ┌────────────────────┐      ┌──────────────────┐
 │ health · rank ·    │ ───▶ │  layout manifest   │ ───▶ │ renderer draws   │
 │ data · habits      │      │  (JSON, allowlist) │      │ known blocks only │
 └────────────────────┘      └────────────────────┘      └──────────────────┘
        Conductor / composer / on-device LLM                shipped in binary
```

**The invariant that keeps it store-legal:** *the AI outputs constrained DATA
from a fixed vocabulary; it never outputs or executes code.* Hold that line and
the app can be infinitely personalized and stay compliant.

---

## The exact rules, and how we satisfy them

### Apple — App Store Review Guideline **2.5.2** + Developer Agreement **3.3.2**
- **Rule:** an app may not download, install, or execute code that "introduces or
  changes features or functionality" or changes the app's primary purpose.
  Interpreted code is allowed *only* when it's run by Apple's built-in
  interpreters (WebKit/JavaScriptCore) as downloaded **resources/data**, and does
  not change the app's advertised purpose.
- **How we comply:** we download **no executable code, ever**. The manifest is
  data. All JS/CSS/render logic ships in the binary and is reviewed. The AI only
  reorders/shows/hides reviewed blocks. The app's purpose — a private health
  companion — never changes per user. ✅

### Google Play — **Device and Network Abuse** + **Deceptive Behavior**
- **Rule:** apps may not download executable code (e.g. dex, native libraries)
  from a source other than Google Play; interpreted languages/config driving the
  app are fine, and the app must do what it says.
- **How we comply:** identical boundary — no downloaded executable code; the
  manifest is data; the app behaves as advertised for every user. ✅

### Both stores — the app is a Capacitor/WebView hybrid
Running our own JS/CSS *inside the app package* in a WebView is explicitly fine
and universal (every hybrid app does it). What's forbidden is *fetching new code
to run* — which we never do.

---

## What ships today (all verified in-browser)

Each of these is the AI/rules reading the user and re-specifying the app,
deterministically rendered:

- **Health → Conductor** — mode (recover/maintain/push) sets the **whole-app
  accent color, tab order, coach tone, and CTA**. (Proven: a run-down body →
  blue Recovery app; a primed body → teal Steady app.)
- **Rank + data + focus → personal shape** — reorders the Today cards, biases the
  coach, labels the app.
- **Habits → usage engine** (`usage.js`) — the **tab bar and menus reorder around
  what you actually use** (proven: Coach/Vault bubbled into the visible tabs
  after heavy use).
- **Data state → composer manifest** (`composer.js`) — **whole blocks appear and
  disappear**: a new user's Today hides Lifeline Age and surfaces "Connect a
  source"; after connecting, Age appears and the surface swaps to "Add your labs."
  (Proven live.)

## The roadmap to "the AI rebuilds the whole app" (same architecture, no new risk)

1. **On-device LLM authors the manifest.** Feed the profile (health + rank + data
   + habits) to the local Gemma model; have it emit the composer's JSON. The
   renderer is unchanged, so compliance is unchanged.
2. **Manifest covers every screen**, not just Today (Coach, Vault, Arena layouts
   become manifest-driven too).
3. **Per-user theming/icons** from the profile (accent families, block styles) —
   still data, still an allowlist.
4. **A validation gate** in the renderer that rejects any manifest key outside the
   allowlist (defense-in-depth: even a compromised/hallucinated manifest can only
   produce known, safe blocks).

**Bottom line:** the world-first thing you're describing is real and shippable —
as long as the AI stays on the *data* side of the line and never crosses into
generating code. This repo is built on that line.
