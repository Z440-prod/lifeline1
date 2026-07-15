/* Lifeline — application shell.
   Hash-routed SPA over the Antigravity backend. Every feature talks to the
   real API: sessions come from /auth/dev-session (development), scores and
   ranks from /game/*, entitlements from /billing/*, documents from /sync/*,
   providers from /integrations/*. */

import { api, connect, account, keepAlive, identity, onConnection, status, deviceCrypto } from './api.js';
import * as engine from './engine.js';
import * as charts from './charts.js';
import { sound, armGlobalSounds } from './sound.js';
import { localAI } from './localai.js';
import { TIER_LABEL } from './device.js';
import { notify } from './notify.js';
import { installNativeBridges } from './native-bridge.js';
import { mountFeelSlider } from './feelslider.js';
import { usage } from './usage.js';

// Wire the native capability bridges (IAP, notifications, on-device AI, device,
// sign-in, health) the moment the module loads — before boot reads any of them.
// No-ops entirely on the web; lights up inside the iOS/Android shell.
installNativeBridges();

/* Native store shells (Capacitor) must not show web-payment surfaces —
   subscriptions go through StoreKit / Play Billing there, and donations are
   web-only (Apple 3.2.1 / Play Payments). */
const IN_STORE_SHELL = typeof window.Capacitor !== 'undefined';

const $ = (s, el = document) => el.querySelector(s);
const $$ = (s, el = document) => [...el.querySelectorAll(s)];
const esc = (t) => { const d = document.createElement('div'); d.textContent = String(t ?? ''); return d.innerHTML; };

/* ── Store ────────────────────────────────────────────────────────────────── */
const store = {
    insights: null,
    game: null,          // /game/config
    billing: null,       // /billing/config
    profile: null,       // /game/profile (server truth)
    board: null,         // /game/leaderboard
    sub: null,           // /billing/subscription
    beta: null,
    connections: {},     // provider -> connection
    whoopMetrics: null,
    signals: engine.todaySignals(),
    coachLog: [],        // {role, text}
    labMeta: JSON.parse(localStorage.getItem('lifeline.labs') || '{}'),
    journalDrafts: 0,
    localAI: null,       // device scan + on-device model state (localAI.probe())
    anecdote: null,      // { day, text } — the AI-written daily note (cached per day)
};
const saveLabMeta = () => localStorage.setItem('lifeline.labs', JSON.stringify(store.labMeta));

const whoopConnected = () => store.connections.whoop?.status === 'connected';
const vitalityNow = () => store.insights ? engine.vitality(store.insights, store.signals, { whoop: whoopConnected() }) : 0;

/* Effective entitlements: whatever /billing/subscription said, else the free
   defaults. The server enforces all of this independently — the client only
   uses it to show the right doors. */
const FREE_ENTITLEMENTS = { history_days: 7, ai_coach_daily_limit: 3, all_integrations: false, biomarker_tracking: false, competitive_seasons: false, ad_free: false, beta_access: false, early_features: false };
const can = () => store.sub?.entitlements || FREE_ENTITLEMENTS;

/* ── The Conductor ────────────────────────────────────────────────────────────
   "The AI controls the app." Each day the client reduces your own readiness +
   streak to a single *mode* using the rules the server ships in
   insights.conductor, and the mode reshapes the whole app: accent color, which
   view leads the tab bar, the primary call-to-action, and the coach's tone.
   All computed here from data the server never sees — the rhythm is yours.
   Recomputed at most once per local day and cached so the app is stable within
   a day and only shifts as your health does. */
let conductorCache = null; // { day, mode }
function conductorMode() {
    const cfg = store.insights?.conductor;
    if (!cfg) return null;
    const day = new Date().toISOString().slice(0, 10);
    const streakLive = !!store.profile
        && store.profile.streak_days > 0
        && store.profile.last_submission_date === day;
    // A key over the inputs that actually change the mode, so logging a score
    // (which can flip Maintain → Push) re-evaluates immediately, but nothing
    // else churns it.
    const key = `${day}:${streakLive}:${!!store.insights}`;
    if (conductorCache && conductorCache.key === key) return conductorCache.mode;

    let chosen = null;
    if (store.insights) {
        const r = engine.readiness(store.insights, store.signals, { whoop: whoopConnected() });
        for (const m of cfg.modes) {
            const okMin = m.min_readiness == null || r.score >= m.min_readiness;
            const okMax = r.score <= m.max_readiness;
            const okStreak = !m.requires_streak || streakLive;
            if (okMin && okMax && okStreak) { chosen = m; break; }
        }
    }
    if (!chosen) chosen = cfg.modes.find((m) => m.id === cfg.default_mode) || cfg.modes[0];
    conductorCache = { key, mode: chosen };
    return chosen;
}

/* Apply the day's mode to the shell: recolor the accent and reorder the primary
   tab bar so the mode's lead view sits first. Called on every render so the app
   consistently wears the current rhythm. */
function applyConductor() {
    const mode = conductorMode();
    if (!mode) return null;
    document.documentElement.style.setProperty('--tint', mode.accent);
    document.documentElement.setAttribute('data-conductor', mode.id);
    if (Array.isArray(mode.view_order)) {
        ROUTES.sort((a, b) => {
            const ia = mode.view_order.indexOf(a.id);
            const ib = mode.view_order.indexOf(b.id);
            return (ia < 0 ? 99 : ia) - (ib < 0 ? 99 : ib);
        });
    }
    // Habit layer: the Conductor's lead view stays the hero (index 0), but the
    // rest reorder by how much THIS user actually uses them — so a heavy Coach
    // user sees Coach bubble into the visible tab bar, a Vault-first user sees
    // Vault. A fresh user (no history) keeps the Conductor order (stable sort).
    if (ROUTES.length > 2) {
        const lead = ROUTES[0];
        const rest = ROUTES.slice(1);
        const ranked = usage.rank(rest.map((r) => r.id));
        rest.sort((a, b) => ranked.indexOf(a.id) - ranked.indexOf(b.id));
        ROUTES.length = 0;
        ROUTES.push(lead, ...rest);
    }
    return mode;
}

/* The coach's tone for today, as a system-prompt preamble the on-device prompt
   builder prepends so the AI companion's voice matches the mode. */
function conductorTonePrompt() {
    const cfg = store.insights?.conductor;
    const mode = conductorMode();
    if (!cfg || !mode) return '';
    return cfg.tone_prompts?.[mode.coach_tone] || '';
}

/* ── Personal shape ───────────────────────────────────────────────────────────
   The Conductor decides the day's *rhythm* from your health. The personal shape
   layers on the other three things that make each user's app their own: what
   they've uploaded (connected sources + labs), what rank they hold (league
   prestige), and what they've chosen to focus on. Together these reorder the
   Today view, bias the coach, and label the app so no two users see the same
   thing. All computed on-device from data the server never sees. */
const FOCI = {
    auto: { label: 'Auto', signals: [], lead: null },
    sleep: { label: 'Sleep', signals: ['sleep_hours'], lead: 'circadian' },
    heart: { label: 'Heart', signals: ['resting_heart_rate', 'hrv_ms'], lead: 'readiness' },
    activity: { label: 'Activity', signals: ['daily_steps'], lead: 'readiness' },
    longevity: { label: 'Longevity', signals: [], lead: 'age' },
};
const userFocus = () => localStorage.getItem('lifeline.focus') || 'auto';
const setUserFocus = (f) => localStorage.setItem('lifeline.focus', f);

function domainForSignal(key) {
    if (key === 'sleep_hours') return 'sleep';
    if (key === 'resting_heart_rate' || key === 'hrv_ms') return 'heart';
    if (key === 'daily_steps') return 'activity';
    return 'longevity';
}

/* Reduce readiness mode + rank + uploaded data + the Focus setting to one
   render-ready description of *this user's* app right now. */
function personalShape() {
    const cfg = store.insights;
    if (!cfg) return null;
    const mode = conductorMode();
    const s = store.signals;
    const devs = engine.signalDeviations(cfg, s);

    // Focus: the user's explicit choice, or (Auto) their weakest signal today.
    const setting = userFocus();
    const weakest = [...devs].sort((a, b) => a.goodness - b.goodness)[0];
    const focus = setting === 'auto' ? (weakest ? domainForSignal(weakest.key) : 'longevity') : setting;
    const spec = FOCI[focus] || FOCI.longevity;

    // Rank → prestige (0..1 up the league ladder).
    const v = vitalityNow();
    const lg = store.game ? engine.leagueFor(store.game, v) : null;
    const leagues = store.game?.leagues || [];
    const leagueIdx = lg ? leagues.findIndex((l) => l.id === lg.id) : -1;
    const prestige = leagues.length > 1 && leagueIdx >= 0 ? leagueIdx / (leagues.length - 1) : 0;

    // Uploaded data → richness (connected sources + uploaded labs).
    const sources = Object.values(store.connections || {}).filter((c) => c.status === 'connected').length;
    const labs = Object.keys(store.labMeta || {}).length;
    const richScore = sources + (labs > 0 ? 1 : 0) + (labs >= 3 ? 1 : 0);
    const dataRichness = richScore >= 3 ? 'rich' : richScore >= 1 ? 'growing' : 'sparse';

    // What the user actually uses most (recency-weighted), for the coach.
    const labelForView = (id) => (ROUTES.find((r) => r.id === id)?.label) || id;
    const habits = usage.summary(labelForView, ['portrait', 'arena', 'coach', 'vault', 'sources', 'plans', 'settings']);

    const signature = `${mode ? mode.label : 'Steady'} · ${spec.label} focus${lg ? ' · ' + lg.name : ''}`;
    const coachContext = `The user's focus today is ${spec.label}`
        + `${setting === 'auto' ? ' (their weakest signal)' : ' (their choice)'}. `
        + `${lg ? `They compete in the ${lg.name} league. ` : ''}`
        + `${labs > 0 ? `They have uploaded ${labs} lab result(s). ` : ''}`
        + `${habits ? `They spend most of their time in ${habits}. ` : ''}`
        + `Bias your guidance toward ${spec.label.toLowerCase()}.`;

    return {
        mode,
        focus,
        focusAuto: setting === 'auto',
        focusLabel: spec.label,
        focusSignals: spec.signals,
        lead: spec.lead,
        weakestKey: weakest?.key,
        league: lg,
        prestige,
        sources,
        labs,
        dataRichness,
        signature,
        coachContext,
    };
}

/* Today's health signals: real on-device data from HealthKit / Health Connect
   when the native shell exposes window.LifelineHealth, else the deterministic
   browser simulation. Merged so a partial native payload still fills the rest. */
async function refreshSignals() {
    const base = engine.todaySignals();
    try {
        if (typeof window !== 'undefined' && window.LifelineHealth?.read) {
            const real = await window.LifelineHealth.read();
            if (real && typeof real === 'object') return { ...base, ...real };
        }
    } catch { /* fall back to the simulation */ }
    return base;
}

/* ── Daily anecdote ───────────────────────────────────────────────────────────
   Once a day the AI writes a short, personal note about your day — built from
   the personal shape (vitality, focus, standout signal, league, streak). The
   prompt is assembled on-device and answered by the on-device model when
   installed, else the identity-stripping proxy, else an offline template. The
   health numbers never leave the device; only the finished sentence does (to
   the OS notification center, if you've opted in). Cached once per local day. */
function anecdoteStats() {
    const cfg = store.insights;
    const s = store.signals;
    const shape = personalShape();
    const v = vitalityNow();
    const devs = cfg ? engine.signalDeviations(cfg, s) : [];
    const best = [...devs].sort((a, b) => b.goodness - a.goodness)[0];
    const la = cfg ? engine.lifelineAge(cfg, s) : { offset: 0 };
    return {
        shape,
        vitality: v,
        focus: shape?.focusLabel || 'Longevity',
        league: shape?.league?.name || '',
        streak: store.profile?.streak_days ? `${store.profile.streak_days}-day streak` : '',
        best_signal: best?.name || 'recovery',
        age_delta: `${Math.abs(la.offset)} yrs ${la.offset <= 0 ? 'younger' : 'older'}`,
    };
}

/* Fill a template's {tokens} from the stats. */
function fillTemplate(tpl, st) {
    return tpl
        .replace(/\{vitality\}/g, st.vitality)
        .replace(/\{focus\}/g, st.focus.toLowerCase())
        .replace(/\{league\}/g, st.league || 'your league')
        .replace(/\{streak\}/g, st.streak || 'streak')
        .replace(/\{best_signal\}/g, st.best_signal.toLowerCase())
        .replace(/\{age_delta\}/g, st.age_delta)
        .replace(/\s+/g, ' ')
        .trim();
}

/* Deterministic offline anecdote from the server's templates — always works,
   even with no network and no model. Picks a template by mode, seeded by the
   day so it's stable within a day but varies across days. */
function templateAnecdote(st) {
    const a = store.insights?.anecdote;
    const modeId = st.shape?.mode?.id || 'maintain';
    const list = (a?.templates?.[modeId]) || [
        `Vitality ${st.vitality} today — ${st.best_signal.toLowerCase()} led the way. Keep the rhythm going.`,
    ];
    const daySeed = new Date().getDate();
    return fillTemplate(list[daySeed % list.length], st);
}

/* Keep only the first sentence and trim it to a notification-friendly length. */
function oneSentence(text, maxWords = 30) {
    let t = String(text || '').replace(/\s+/g, ' ').trim();
    // Strip a leading role label the model sometimes adds.
    t = t.replace(/^["“]?(lifeline[:,-]\s*)/i, '');
    const m = t.match(/^[^.!?]*[.!?]/);
    if (m) t = m[0];
    const words = t.split(' ');
    if (words.length > maxWords) t = words.slice(0, maxWords).join(' ') + '…';
    return t.trim();
}

/* Produce today's anecdote (cached per day). Order: on-device model → proxy →
   template. Never throws — always returns a usable sentence. */
async function generateDailyAnecdote(force = false) {
    const day = new Date().toISOString().slice(0, 10);
    if (!force && store.anecdote?.day === day && store.anecdote.text) return store.anecdote.text;

    const st = anecdoteStats();
    const cfg = store.insights?.anecdote;
    const system = cfg?.system || 'Write one warm, specific sentence about the user\'s day from the stats.';
    const statLine = `vitality ${st.vitality}, ${st.focus} focus, standout ${st.best_signal}`
        + `${st.league ? ', ' + st.league + ' league' : ''}${st.streak ? ', ' + st.streak : ''}, `
        + `biological age ${st.age_delta} than passport`;
    const prompt = `${system}\n\nToday's stats: ${statLine}.`;
    const maxWords = cfg?.max_words || 28;

    let text = null;
    // 1. On-device model (fully offline, most private).
    try {
        const local = await localAI.generate(prompt, { system, signals: store.signals, summary: statLine });
        if (local) text = oneSentence(local, maxWords);
    } catch { /* fall through */ }
    // 2. Identity-stripping proxy. Ignore the development mock echo (returned
    //    when no API key is configured) so dev/offline gets the real template.
    if (!text && status().authed) {
        try {
            const ch = await api.challenge();
            const res = await api.aiProxy(prompt, ch.data?.challenge || 'token');
            if (res.status === 200) {
                const raw = res.data?.content?.[0]?.text || res.data?.content;
                if (raw && !String(raw).includes('[Mock Claude Response]')) {
                    text = oneSentence(raw, maxWords);
                }
            }
        } catch { /* fall through */ }
    }
    // 3. Offline template.
    if (!text) text = templateAnecdote(st);

    store.anecdote = { day, text };
    return text;
}

/* Once a local day, when enabled, fire the daily note as an OS notification.
   Also (re)arms the native background schedule. Safe to call on every boot. */
async function maybeSendDailyNotification() {
    try {
        notify.scheduleDaily(); // keep the native daily reminder armed
        if (!notify.enabled() || notify.sentToday()) return;
        if (notify.permission() !== 'granted' && !window.LifelineNotifications) return;
        const text = await generateDailyAnecdote();
        await notify.show(store.insights?.anecdote?.notification_title || 'Your Lifeline is ready', text);
    } catch { /* notifications are best-effort; never block the app */ }
}

/* ── Toasts ───────────────────────────────────────────────────────────────── */
function toast(text, tone = 'var(--ok)') {
    let host = $('.toasts');
    if (!host) { host = document.createElement('div'); host.className = 'toasts'; document.body.appendChild(host); }
    const el = document.createElement('div');
    el.className = 'toast';
    el.innerHTML = `<span class="d" style="background:${tone}"></span>${esc(text)}`;
    host.appendChild(el);
    setTimeout(() => { el.style.opacity = '0'; el.style.transition = 'opacity .3s'; setTimeout(() => el.remove(), 320); }, 2600);
}

/* ── Icons ────────────────────────────────────────────────────────────────── */
const I = {
    portrait: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9"><path d="M3 13h4l3-8 4 14 3-6h4" stroke-linecap="round" stroke-linejoin="round"/></svg>',
    arena: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9"><path d="M7 4h10v3a5 5 0 0 1-10 0V4z"/><path d="M5 5H3v1a3 3 0 0 0 3 3M19 5h2v1a3 3 0 0 1-3 3M9 14.5V17h6v-2.5M8 20h8" stroke-linecap="round"/></svg>',
    coach: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9"><path d="M21 12a8 8 0 0 1-11.5 7.2L4 21l1.8-5.5A8 8 0 1 1 21 12z"/></svg>',
    vault: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9"><rect x="3" y="4" width="18" height="16" rx="3"/><path d="M3 9.5h18M9.5 4v16" stroke-linecap="round"/></svg>',
    sources: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9"><circle cx="6" cy="12" r="2.6"/><circle cx="18" cy="6" r="2.6"/><circle cx="18" cy="18" r="2.6"/><path d="M8.3 10.7l7.4-3.4M8.3 13.3l7.4 3.4"/></svg>',
    plans: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9"><rect x="3" y="5" width="18" height="14" rx="3"/><path d="M3 10h18M7 15h4" stroke-linecap="round"/></svg>',
    settings: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9"><circle cx="12" cy="12" r="3.2"/><path d="M19 12a7 7 0 0 0-.1-1.2l2-1.5-2-3.4-2.3 1a7 7 0 0 0-2-1.2L14.2 3h-4l-.4 2.7a7 7 0 0 0-2 1.2l-2.3-1-2 3.4 2 1.5a7 7 0 0 0 0 2.4l-2 1.5 2 3.4 2.3-1a7 7 0 0 0 2 1.2l.4 2.7h4l.4-2.7a7 7 0 0 0 2-1.2l2.3 1 2-3.4-2-1.5c.06-.4.1-.8.1-1.2z" stroke-linejoin="round"/></svg>',
    check: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6"><path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/></svg>',
    lock: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9"><rect x="5" y="11" width="14" height="9" rx="2.5"/><path d="M8 11V8a4 4 0 0 1 8 0v3" stroke-linecap="round"/></svg>',
    flame: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M12 22c4.4 0 7.5-2.9 7.5-7.2 0-3.1-1.7-5.4-3.4-7.3-.4 1.2-1.2 2.3-2.2 2.9.2-2.5-.8-6-3.9-8.4.3 3-1.2 4.6-2.7 6.4C5.7 10.2 4.5 12 4.5 14.8 4.5 19.1 7.6 22 12 22zm0-2.2c-1.8 0-3-1.2-3-2.9 0-1.3.7-2.1 1.5-3 .6-.7 1.3-1.4 1.6-2.4 1.7 1.2 2.9 3.2 2.9 5C15 18.6 13.8 19.8 12 19.8z"/></svg>',
    eye: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9"><path d="M2 12s3.5-6.5 10-6.5S22 12 22 12s-3.5 6.5-10 6.5S2 12 2 12z"/><circle cx="12" cy="12" r="2.6"/><path d="M4 4l16 16" stroke-linecap="round"/></svg>',
    device: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9"><rect x="7" y="2.5" width="10" height="19" rx="2.8"/><path d="M11 18.5h2" stroke-linecap="round"/></svg>',
    logo: '<svg viewBox="0 0 32 32"><rect width="32" height="32" rx="8" fill="var(--surface)" stroke="var(--hairline)"/><path d="M5 18h5.5l2.8-7.5 3.8 11 2.8-5.5H27" fill="none" stroke="var(--pulse)" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round"/></svg>',
};
const PIGMENT = { cardio: 'var(--cardio)', sleep: 'var(--sleep)', activity: 'var(--activity)', energy: 'var(--energy)', recovery: 'var(--recovery)' };

/* ── Routes ───────────────────────────────────────────────────────────────── */
const ROUTES = [
    { id: 'portrait', label: 'Today', icon: I.portrait },
    { id: 'arena', label: 'Arena', icon: I.arena },
    { id: 'coach', label: 'Coach', icon: I.coach },
    { id: 'vault', label: 'Vault', icon: I.vault },
    { id: 'sources', label: 'Sources', icon: I.sources },
    { id: 'plans', label: 'Plans', icon: I.plans },
    { id: 'settings', label: 'Settings', icon: I.settings },
];
const MORE_ROUTES = ['sources', 'plans', 'settings'];
const routeId = () => (location.hash.replace(/^#\/?/, '') || 'portrait').split('?')[0];
const TITLES = { portrait: 'Today', arena: 'The Arena', coach: 'Coach', vault: 'The Vault', sources: 'Sources', plans: 'Plans', settings: 'Settings' };

/* ── Frame ────────────────────────────────────────────────────────────────── */
const MORE_ICON = '<svg viewBox="0 0 24 24" fill="currentColor"><circle cx="5" cy="12" r="2.1"/><circle cx="12" cy="12" r="2.1"/><circle cx="19" cy="12" r="2.1"/></svg>';

function renderFrame() {
    $('#app').innerHTML = `
    <div class="frame">
        <header class="topbar" id="topbar"><span class="t" id="topbarTitle">Today</span><span class="dot" id="topbarDot"></span></header>
        <main class="main"><div class="content" id="view"></div></main>
        <nav class="tabbar" id="tabbar"></nav>
    </div>`;
    paintTabbar();
    onConnection((s) => {
        const dot = $('#topbarDot');
        if (dot) dot.classList.toggle('on', s.online && s.authed);
    });
    // Scroll-edge effect: the large title hands off to a compact glass bar.
    window.addEventListener('scroll', () => {
        $('#topbar')?.classList.toggle('showing', window.scrollY > 74);
    }, { passive: true });
}

/* (Re)build the primary tab bar to match the current ROUTES order — the
   Conductor may have reordered it so today's lead view sits first. */
let tabbarOrder = '';
function paintTabbar() {
    const nav = $('#tabbar');
    if (!nav) return;
    const order = ROUTES.slice(0, 4).map((r) => r.id).join(',');
    if (order === tabbarOrder && nav.children.length) return; // unchanged
    tabbarOrder = order;
    nav.innerHTML = `${ROUTES.slice(0, 4).map((r) => `<button class="tab" data-nav="${r.id}">${r.icon}<span>${r.label}</span></button>`).join('')}
        <button class="tab" id="moreTab">${MORE_ICON}<span>More</span></button>`;
    $$('[data-nav]', nav).forEach((b) => b.addEventListener('click', () => { usage.record(b.dataset.nav); location.hash = `#/${b.dataset.nav}`; }));
    $('#moreTab', nav).addEventListener('click', openMoreSheet);
    setActiveNav(routeId());
}

function setActiveNav(id) {
    $$('[data-nav]').forEach((b) => b.classList.toggle('active', b.dataset.nav === id));
    $('#moreTab')?.classList.toggle('active', MORE_ROUTES.includes(id));
    const t = $('#topbarTitle');
    if (t) t.textContent = TITLES[id] || 'Lifeline';
}

/* iOS bottom sheet for the secondary destinations. */
function openMoreSheet() {
    const host = $('#overlays');
    const rowDefs = {
        sources: { id: 'sources', name: 'Sources', sub: 'Apple · Google · Whoop', color: 'var(--sleep)', icon: I.sources },
        plans: { id: 'plans', name: 'Plans', sub: store.sub?.tier === 'free' || !store.sub ? 'Free plan' : `${cap(store.sub.tier)} member`, color: 'var(--recovery)', icon: I.plans },
        settings: { id: 'settings', name: 'Settings', sub: 'Identity, theme & privacy', color: 'var(--ink-3)', icon: I.settings },
    };
    // Order the secondary destinations by how often this user opens them.
    const rows = usage.rank(Object.keys(rowDefs)).map((id) => rowDefs[id]);
    host.innerHTML = `
        <div class="sheet-dim" id="sheetDim"></div>
        <div class="sheet" role="dialog" aria-label="More">
            <div class="grab"></div>
            <h3>More</h3>
            <div class="sheet-group">
                ${rows.map((r) => `<button class="sheet-row" data-go="${r.id}">
                    <span class="mark" style="background:${r.color}">${r.icon}</span>
                    <span>${r.name}<br><small style="color:var(--ink-3); font-weight:500; font-size:var(--fs-micro)">${esc(r.sub)}</small></span>
                    <svg class="chev" width="8" height="14" viewBox="0 0 8 14" fill="none" stroke="currentColor" stroke-width="2"><path d="M1 1l6 6-6 6" stroke-linecap="round"/></svg>
                </button>`).join('')}
            </div>
        </div>`;
    const close = () => { host.innerHTML = ''; };
    $('#sheetDim').addEventListener('click', close);
    $$('.sheet-row', host).forEach((b) => b.addEventListener('click', () => { usage.record(b.dataset.go); close(); location.hash = `#/${b.dataset.go}`; }));
}

/* Confetti — because logging your score should feel like something. */
function confetti(x = innerWidth / 2, y = innerHeight * 0.7) {
    const colors = ['var(--tint)', 'var(--energy)', 'var(--activity)', 'var(--sleep)', 'var(--recovery)'];
    for (let i = 0; i < 26; i++) {
        const bit = document.createElement('span');
        bit.className = 'confetti-bit';
        const ang = (Math.random() - 0.5) * Math.PI;
        const dist = 90 + Math.random() * 200;
        bit.style.cssText = `left:${x}px; top:${y}px; background:${colors[i % colors.length]};` +
            `--dx:${Math.sin(ang) * dist}px; --dy:${140 + Math.random() * 260}px;` +
            `--rot:${(Math.random() - 0.5) * 900}deg; --dur:${0.9 + Math.random() * 0.7}s;`;
        document.body.appendChild(bit);
        setTimeout(() => bit.remove(), 1800);
    }
    if (navigator.vibrate) navigator.vibrate(18);
}

function offlineBanner() {
    return status().online ? '' : `<div class="offline-banner">⚠︎ <span><b>Backend offline.</b> Start it with <span class="tnum">cargo run</span> — the app reconnects automatically.</span></div>`;
}

/* ═══ VIEW: PORTRAIT ═══════════════════════════════════════════════════════ */
function viewPortrait(el) {
    const cfg = store.insights;
    const s = store.signals;
    const hour = new Date().getHours();
    const hello = hour < 12 ? 'Good morning' : hour < 17 ? 'Good afternoon' : 'Good evening';
    if (!cfg) { el.innerHTML = offlineBanner() + '<div class="empty">Waiting for the insights rulebook…</div>'; return; }

    const v = vitalityNow();
    const r = engine.readiness(cfg, s, { whoop: whoopConnected() });
    const la = engine.lifelineAge(cfg, s);
    const devs = engine.signalDeviations(cfg, s);
    const corr = engine.correlations(cfg);
    const chrono = engine.chronotype(cfg, s);
    const lg = store.game ? engine.leagueFor(store.game, v) : null;
    const dateStr = new Date().toLocaleDateString(undefined, { weekday: 'long', month: 'long', day: 'numeric' });
    // Loss aversion, honestly applied: only warn about a streak that exists.
    const todayIso = new Date().toISOString().slice(0, 10);
    const loggedToday = store.profile?.last_submission_date === todayIso;
    const streakAtRisk = !!store.profile && !loggedToday && store.profile.streak_days > 0 && can().competitive_seasons;
    const mode = conductorMode();
    const shape = personalShape();

    // Signals reordered so the focused domain leads and is highlighted.
    const orderedDevs = [...devs].sort((a, b) => {
        const af = shape.focusSignals.includes(a.key) ? 0 : 1;
        const bf = shape.focusSignals.includes(b.key) ? 0 : 1;
        return af - bf;
    });

    // The three insight cards, reordered so the focus leads the Today view —
    // this is a visible way "what you focus on" reshapes the app per user.
    const insightCards = {
        readiness: `
        <div class="card col-4 ${shape.lead === 'readiness' ? 'focus-card' : ''}">
            <div class="card-title">Readiness${shape.lead === 'readiness' ? ' <span class="focus-tag">focus</span>' : ''}</div>
            <div class="card-sub">fused from every connected source</div>
            <div style="display:flex; justify-content:center;">${charts.ringGauge({ value: r.score, label: r.label })}</div>
            <p style="text-align:center; font-size:var(--fs-small); color:var(--ink-2); margin-top:10px;">${esc(r.driver)}</p>
        </div>`,
        age: `
        <div class="card col-4 ${shape.lead === 'age' ? 'focus-card' : ''}">
            <div class="card-title">Lifeline Age${shape.lead === 'age' ? ' <span class="focus-tag">focus</span>' : ''}</div>
            <div class="card-sub">transparent additive model — inspect it in Settings</div>
            <div class="tiles">
                <div class="tile"><div class="v tnum">${la.age}</div><div class="l">your body says</div>
                    <div class="delta" style="color:${la.offset <= 0 ? 'var(--ok)' : 'var(--warn-deep)'}">${Math.abs(la.offset)} yrs ${la.offset <= 0 ? 'younger' : 'older'} than your passport</div></div>
                <div class="tile"><div class="v tnum">${s.chrono_age}</div><div class="l">your passport says</div></div>
            </div>
        </div>`,
        circadian: `
        <div class="card col-4 ${shape.lead === 'circadian' ? 'focus-card' : ''}">
            <div class="card-title">Circadian window${shape.lead === 'circadian' ? ' <span class="focus-tag">focus</span>' : ''}</div>
            <div class="card-sub">${esc(chrono.type)} chronotype · shifted to your sleep midpoint</div>
            ${charts.circadianTrack(chrono.windows, 420)}
        </div>`,
    };
    const cardOrder = [shape.lead, 'readiness', 'age', 'circadian']
        .filter((k, i, a) => k && insightCards[k] && a.indexOf(k) === i);

    el.innerHTML = `
    ${offlineBanner()}
    <div class="page-head">
        <div class="eyebrow">${esc(dateStr)}</div>
        <h1>${hello}.</h1>
        <div class="sub">Drawn fresh on your device, from your signals. The server never sees a heartbeat.</div>
    </div>
    ${mode ? `<div class="conductor-banner" data-mode="${mode.id}">
        <div class="cb-glyph" style="background:${mode.accent}"></div>
        <div class="cb-copy">
            <div class="cb-label">${esc(mode.label)}</div>
            <div class="cb-sub">${esc(mode.subtitle)}</div>
        </div>
        <button class="btn btn-pulse btn-sm cb-cta" id="conductorCta">${esc(mode.primary_cta.text)}</button>
    </div>` : ''}
    <div class="shape-chips" title="Your app is shaped by your health, your rank, your data, and your focus">
        ${mode ? `<span class="chip shape-mode"><span class="d" style="background:${mode.accent}"></span>${esc(mode.label)}</span>` : ''}
        <button class="chip shape-focus" id="focusChip">${esc(shape.focusLabel)} focus${shape.focusAuto ? ' · auto' : ''}</button>
        ${shape.league ? `<span class="chip shape-league"><span class="d" style="background:${charts.LEAGUE_COLORS[shape.league.id]}"></span>${esc(shape.league.name)}</span>` : ''}
        <span class="chip shape-data" data-rich="${shape.dataRichness}">${esc(shape.dataRichness)} data</span>
    </div>
    <div class="daily-note" id="dailyNote">
        <span class="dn-mark">${I.coach}</span>
        <span class="dn-text">${store.anecdote?.text ? esc(store.anecdote.text) : 'Writing your note for today…'}</span>
    </div>
    ${streakAtRisk ? `<div class="streak-warn"><span class="flame">${I.flame}</span><span>Your <b>${store.profile.streak_days}-day streak</b> is on the line — log today's score to keep it alive.</span></div>` : ''}
    <div class="grid">
        <div class="card hero col-12">
            <div class="vitality-hero">
                <div class="vitality-num">
                    <span class="n tnum">${v}</span>
                    <span class="cap">today's vitality — the only number that ever leaves your hands</span>
                </div>
                <div class="vitality-trace">${charts.pulseTrace({ vitality: v, rhr: s.resting_heart_rate })}</div>
                <div class="vitality-side">
                    ${lg ? `<span class="chip sticker"><span class="d" style="background:${charts.LEAGUE_COLORS[lg.id]}"></span>${esc(lg.name)} league</span>` : ''}
                    ${store.profile?.streak_days ? `<span class="chip"><span class="flame">${I.flame}${store.profile.streak_days}</span>&nbsp;day streak</span>` : ''}
                    ${loggedToday
                        ? `<button class="btn btn-ghost btn-sm" id="logScoreBtn">Logged today ✓ · see the Arena</button>`
                        : `<button class="btn btn-pulse btn-sm" id="logScoreBtn">Log today's score</button>`}
                </div>
            </div>
        </div>

        <div class="card col-12 feel-card" id="feelMount"></div>

        ${cardOrder.map((k) => insightCards[k]).join('')}

        <div class="card col-7">
            <div class="card-title">Signals vs ideal <span class="hint">band tables from /insights/config</span></div>
            <div class="card-sub">bar length = how close each signal sits to its optimal band · your <b>${esc(shape.focusLabel.toLowerCase())}</b> focus leads</div>
            <div class="signal-rows">
                ${orderedDevs.map((d) => `
                <div class="signal-row ${shape.focusSignals.includes(d.key) ? 'focused' : ''}" title="${esc(d.name)}: ${d.value}${d.unit} → ${d.years <= 0 ? '' : '+'}${d.years} yrs on Lifeline Age">
                    <span class="name"><span class="d" style="background:${PIGMENT[d.pigment]}"></span>${esc(d.name)}</span>
                    <div class="meter"><i style="width:${Math.round(d.goodness * 100)}%; background:${PIGMENT[d.pigment]}"></i></div>
                    <span class="val tnum">${d.value}<small>${d.unit ? ' ' + d.unit : ''}</small></span>
                </div>`).join('')}
            </div>
        </div>

        <div class="card col-5">
            <div class="card-title">What's moving your score</div>
            <div class="card-sub">habit ↔ outcome correlation, refined on-device</div>
            <div class="corr-list">
                ${corr.map((h, i) => `
                <div class="corr-item ${i === 0 ? 'lead' : ''}" title="${esc(h.name)} · prior ${h.prior}">
                    <span class="name">${esc(h.name)}${i === 0 ? ' ✦' : ''}</span>
                    <div class="meter"><i style="width:${Math.round((h.prior / corr[0].prior) * 100)}%; background:${['var(--recovery)', 'var(--activity)', 'var(--sleep)', 'var(--energy)'][i % 4]}"></i></div>
                    <span class="pct tnum">.${String(Math.round(h.prior * 100)).padStart(2, '0')}</span>
                </div>`).join('')}
            </div>
        </div>

        <div class="col-12"><div class="note"><b>Zero-knowledge by design.</b> These panels are computed here, in your browser, from the rules the server publishes. Your raw signals never leave the device — only today's opaque vitality integer does, and only if you log it to the Arena.</div></div>
    </div>`;

    $('#logScoreBtn')?.addEventListener('click', () => {
        if (loggedToday) { location.hash = '#/arena'; return; }
        submitScoreFlow();
    });

    // Tactile daily check-in. Drag to log how you feel — detent ticks, a light
    // haptic, and a release burst. Persists per day; celebrates the first log.
    mountFeelSlider($('#feelMount'), {
        value: (() => {
            if (store.feel != null) return store.feel;
            try { const r = JSON.parse(localStorage.getItem('lifeline.feel') || 'null'); if (r && r.day === new Date().toDateString()) { store.feel = r.val; return r.val; } } catch (e) { /* private mode */ }
            return 50;
        })(),
        onCommit: (val, label) => {
            const first = store.feel == null;
            store.feel = val;
            try { localStorage.setItem('lifeline.feel', JSON.stringify({ day: new Date().toDateString(), val })); } catch (e) { /* private mode */ }
            toast(`Felt ${label.toLowerCase()} · ${val} — noted for today`);
            if (first && !matchMedia('(prefers-reduced-motion: reduce)').matches) confetti();
        },
    });

    // Tapping the focus chip jumps to the Focus control in Settings.
    $('#focusChip')?.addEventListener('click', () => { location.hash = '#/settings'; });

    // Fill (or refresh) today's AI note. Cached per day; regenerated if the
    // day rolled over since it was last written.
    generateDailyAnecdote().then((text) => {
        const t = $('#dailyNote .dn-text');
        if (t) t.textContent = text;
    }).catch(() => {});

    // The Conductor's call-to-action routes to the mode's suggested view (or
    // logs today's score when the mode wants a check-in).
    $('#conductorCta')?.addEventListener('click', () => {
        const target = mode?.primary_cta?.view;
        if (target === 'portrait' && !loggedToday) { submitScoreFlow(); return; }
        if (target && target !== 'portrait') { location.hash = `#/${target}`; return; }
        location.hash = '#/arena';
    });

    // Count-up on the hero number — the little dopamine ramp.
    if (!matchMedia('(prefers-reduced-motion: reduce)').matches) {
        const numEl = $('.vitality-num .n', el);
        if (numEl) {
            const start = performance.now(), dur = 700;
            const step = (t) => {
                const k = Math.min(1, (t - start) / dur);
                numEl.textContent = Math.round(v * (1 - Math.pow(1 - k, 3)));
                if (k < 1) requestAnimationFrame(step);
            };
            requestAnimationFrame(step);
        }
    }
}

/* ═══ VIEW: ARENA ══════════════════════════════════════════════════════════ */
async function refreshArena() {
    // The leaderboard response embeds the caller's own standing as `me`
    // (same shape as /game/profile), so one call refreshes both — and a
    // fresh device avoids an expected-404 on the profile endpoint.
    const b = await api.leaderboard();
    store.board = b.status === 200 ? b.data : null;
    if (store.board) store.profile = store.board.me && store.board.me.handle ? store.board.me : null;
}

async function submitScoreFlow(handle) {
    const v = vitalityNow();
    if (!can().competitive_seasons) {
        toast('Competing is a Pro feature — pick a plan to enter the Arena', 'var(--warn-deep)');
        location.hash = '#/plans';
        return;
    }
    if (!store.profile && !handle) {
        location.hash = '#/arena';
        toast('Claim a handle first to join the Arena', 'var(--warn)');
        return;
    }
    const res = await api.submitScore(v, handle);
    if (res.status === 200) {
        store.profile = res.data;
        confetti();
        sound.whoosh(); sound.pop(6); sound.chime();
        // Variable reward: the numbers are always honest, the praise rotates.
        const praise = ['Clean.', 'Machine.', 'That’s a statement.'][Math.floor(Math.random() * 3)];
        toast(`${praise} ${v} logged — ${cap(res.data.league)} league, #${res.data.rank} worldwide`);
        await refreshArena();
        if (routeId() === 'arena' || routeId() === 'portrait') render();
    } else if (res.status === 403) {
        toast(res.data?.error?.message || 'Upgrade to compete', 'var(--warn-deep)');
        location.hash = '#/plans';
    } else if (res.status === 409) {
        toast(res.data?.error?.message || 'Handle taken', 'var(--err)');
    } else {
        toast(res.data?.error?.message || `Score rejected (${res.status})`, 'var(--err)');
    }
}

function viewArena(el) {
    const p = store.profile;
    const g = store.game;
    const board = store.board;

    const emblem = p ? charts.leagueEmblem(p.league, p.level) : charts.leagueEmblem('bronze', 1);
    const xpInto = p ? Number(p.xp_into_level) : 0;
    const xpSpan = p ? Math.max(1, Number(p.xp_for_next_level)) : 1;

    el.innerHTML = `
    ${offlineBanner()}
    <div class="page-head">
        <div class="eyebrow">Season ${esc(board?.season_id || g?.season?.current || '—')} · resets weekly</div>
        <h1>The Arena.</h1>
        <div class="sub">A global ladder ranked on health itself. Rivals see your handle, league, and score — never a single biometric.</div>
    </div>
    <div class="grid">
        <div class="card col-6">
            <div class="arena-hero">
                <div class="emblem">${emblem}</div>
                <div class="arena-meta">
                    ${p ? `
                    <div class="league">${esc(cap(p.league))}</div>
                    <div class="standing">as <b>${esc(p.handle)}</b> · level ${p.level} · <span class="flame">${I.flame}${p.streak_days}-day streak</span></div>
                    <div class="standing">rank <b>#${p.rank}</b> of ${p.population} worldwide · top ${Math.max(1, Math.round(100 - p.percentile) || 1)}% · best ${p.best_vitality_score}</div>
                    <div class="xp-wrap">
                        <div class="meter" style="height:8px"><i style="width:${Math.min(100, Math.round((xpInto / xpSpan) * 100))}%; background:linear-gradient(90deg, var(--energy), var(--pulse))"></i></div>
                        <div class="xp-caption"><span>level ${p.level}</span><span>${Math.max(0, xpSpan - xpInto)} XP → ${p.level + 1}</span></div>
                    </div>` : can().competitive_seasons ? `
                    <div class="league">Unranked</div>
                    <div class="standing">Claim a handle and log your first vitality score to enter the global ladder.</div>
                    <div style="display:flex; gap:8px; margin-top:12px; max-width:340px;">
                        <input class="field" id="handleInput" maxlength="20" placeholder="pick a handle (3–20, a–z 0–9 _)" spellcheck="false">
                        <button class="btn btn-pulse" id="joinBtn">Join</button>
                    </div>` : `
                    <div class="league">Spectator</div>
                    <div class="standing">The free plan watches the ladder. <b>Pro</b> competes on it — score, streaks, leagues, weekly seasons.</div>
                    <div style="margin-top:12px;"><button class="btn btn-pulse" id="unlockArenaBtn">Unlock the Arena</button></div>`}
                </div>
            </div>
            ${p && can().competitive_seasons ? `<div style="margin-top:16px; display:flex; gap:9px; flex-wrap:wrap;">
                <button class="btn btn-pulse" id="logBtn">Log today's score · ${vitalityNow()}</button>
                <button class="btn btn-ghost" id="flexBtn">Flex it 💪</button>
            </div>` : ''}
        </div>

        <div class="card col-6">
            <div class="card-title">Today's stats</div>
            <div class="card-sub">what your next submission carries</div>
            <div class="tiles">
                <div class="tile"><div class="v tnum">${vitalityNow()}</div><div class="l">vitality</div></div>
                <div class="tile"><div class="v tnum">${p ? p.streak_days : 0}</div><div class="l">day streak</div></div>
                <div class="tile"><div class="v tnum">${p ? Number(p.season_xp).toLocaleString() : 0}</div><div class="l">season xp</div></div>
            </div>
            <div class="note" style="margin-top:14px;"><b>One submission a day counts.</b> XP = 40 + 2·score + 5·streak (streak capped at 30). Leagues are score bands: ${g ? g.leagues.map((l) => `${l.name} ${l.min_score}+`).join(' · ') : '—'}.</div>
        </div>

        <div class="card col-12">
            <div class="card-title">Global leaderboard <span class="hint">🌍 every device on earth · one ladder · live</span></div>
            <div class="board" id="board">
                ${boardRows(board, p)}
            </div>
        </div>
    </div>`;

    $('#joinBtn')?.addEventListener('click', async () => {
        const h = $('#handleInput').value.trim();
        if (!/^[A-Za-z0-9_]{3,20}$/.test(h)) { toast('Handle must be 3–20 letters, numbers, or _', 'var(--warn)'); return; }
        await submitScoreFlow(h);
    });
    $('#unlockArenaBtn')?.addEventListener('click', () => { location.hash = '#/plans'; });
    $('#flexBtn')?.addEventListener('click', async () => {
        const me = store.profile;
        if (!me) return;
        const line = `I'm #${me.rank} worldwide on Lifeline — ${cap(me.league)} league, vitality ${me.vitality_score}, ${me.streak_days}-day streak. Come take it from me. 🫀`;
        confetti(); sound.chime();
        try {
            if (navigator.share) { await navigator.share({ text: line }); toast('Flexed. 💪'); }
            else { await navigator.clipboard.writeText(line); toast('Flex copied — paste it anywhere 💪'); }
        } catch { /* user dismissed the share sheet */ }
    });
    $('#handleInput')?.addEventListener('keydown', (e) => { if (e.key === 'Enter') $('#joinBtn').click(); });
    $('#logBtn')?.addEventListener('click', () => submitScoreFlow());
}

function boardRows(board, me) {
    const entries = board?.entries || [];
    if (!entries.length) return '<div class="empty">No one has logged a score this season yet.<br>Claim a handle and take rank #1.</div>';
    const rows = entries.map((e) => `
        <div class="board-row ${e.rank <= 3 ? 'podium' : ''} ${me && e.handle === me.handle ? 'me' : ''}">
            <span class="rk tnum">${e.rank}</span>
            <span class="lg" style="background:${charts.LEAGUE_COLORS[e.league] || 'var(--ink-3)'}"></span>
            <span class="who">${esc(e.handle)}${me && e.handle === me.handle ? ' ✦' : ''}<small>lv ${e.level} · ${esc(cap(e.league))}</small></span>
            <span class="xp tnum">${Number(e.season_xp).toLocaleString()} xp</span>
        </div>`).join('');
    const cap_ = entries.length < 3 ? '<div class="ghost-cap">The season is young — every handle above is a real device. Invite your rivals.</div>' : '';
    return rows + cap_;
}

/* ═══ VIEW: COACH ══════════════════════════════════════════════════════════ */
function viewCoach(el) {
    const onDevice = localAI.ready();
    const badge = onDevice
        ? `<span class="ai-badge on-device" title="Answered by a model running entirely on this device">${I.device} on-device · offline</span>`
        : `<span class="ai-badge cloud" title="Answered via the identity-stripping privacy proxy">${I.lock} private proxy</span>`;
    const sysLine = onDevice
        ? 'running fully on your device — no network, nothing leaves your phone'
        : 'end-to-end private — the proxy strips your identity before the model sees a word';
    const eligible = store.localAI?.eligible && !onDevice;

    el.innerHTML = `
    ${offlineBanner()}
    <div class="page-head">
        <div class="eyebrow">Clinical-first · zero retention</div>
        <h1>Coach.</h1>
        <div class="sub">Every message is answered with your on-device context. ${onDevice ? 'Right now it never leaves your phone.' : 'The proxy strips your identity before any model sees a word.'}</div>
    </div>
    <div class="card">
        <div class="coach-head">${badge}</div>
        <div class="coach-thread" id="thread">
            <div class="msg sys">${sysLine}</div>
            ${store.coachLog.map(msgHtml).join('')}
        </div>
        ${eligible ? `<button class="ondevice-offer" id="coachOfferAi">${I.device}<span><b>Run Lifeline AI privately on this device.</b> Download a small model once — then the coach works offline, forever.</span><span class="chev">›</span></button>` : ''}
        <div class="coach-compose">
            <input class="field" id="coachInput" placeholder="Ask about your sleep, labs, training…" autocomplete="off">
            <button class="btn btn-pulse" id="coachSend">Send</button>
        </div>
        <div class="suggest">
            ${['How is my resting heart rate trending?', 'What should I fix first for my Lifeline Age?', 'Plan tomorrow around my chronotype'].map((q) => `<button data-q="${esc(q)}">${esc(q)}</button>`).join('')}
        </div>
    </div>`;

    const thread = $('#thread');
    thread.scrollTop = thread.scrollHeight;
    const send = async (text) => {
        if (!text) return;
        $('#coachInput').value = '';
        store.coachLog.push({ role: 'user', text });
        thread.insertAdjacentHTML('beforeend', msgHtml({ role: 'user', text }));
        const tid = 't' + Date.now();
        thread.insertAdjacentHTML('beforeend', `<div class="msg ai" id="${tid}"><div class="who">LIFELINE COACH</div><div class="typing"><span></span><span></span><span></span></div></div>`);
        thread.scrollTop = thread.scrollHeight;

        // The Conductor sets the coach's voice for the day; the personal shape
        // biases it to this user's focus, rank, and uploaded data — so the same
        // question gets a different answer for a Sleep-focused Gold-league user
        // than for an Activity-focused newcomer.
        const tone = conductorTonePrompt();
        const md = conductorMode();
        const shape = personalShape();
        const preamble = [
            tone ? `Today's coaching tone — ${md.label}: ${tone}` : '',
            shape?.coachContext || '',
        ].filter(Boolean).join(' ');
        const framed = preamble ? `[${preamble}]\n\nUser: ${text}` : text;

        let reply;
        // Prefer the on-device model when it's installed: instant, private, and
        // works with no connection at all. Falls back to the cloud proxy only
        // when there's no local model ready.
        const local = await localAI.generate(framed, {
            system: store.localAI?.catalog?.system_prompt,
            signals: store.signals,
            summary: `readiness context: HRV ${store.signals.hrv_ms}ms, RHR ${store.signals.resting_heart_rate}bpm, sleep ${store.signals.sleep_hours}h, steps ${store.signals.daily_steps}`,
        });
        if (local != null) {
            reply = local;
        } else {
            try {
                const ch = await api.challenge();
                const res = await api.aiProxy(framed, ch.data?.challenge || 'token');
                reply = res.status === 200
                    ? (res.data?.content?.[0]?.text || res.data?.content || 'Understood.')
                    : res.status === 403
                        ? `${res.data?.error?.message || 'Daily free limit reached.'} You'll find the plans under More → Plans.`
                        : `The proxy answered ${res.status}: ${res.data?.error?.message || 'unavailable'}.`;
            } catch { reply = 'Could not reach the backend — and no on-device model is installed yet. Enable on-device AI in Settings to chat offline.'; }
        }
        document.getElementById(tid)?.remove();
        store.coachLog.push({ role: 'ai', text: reply });
        thread.insertAdjacentHTML('beforeend', msgHtml({ role: 'ai', text: reply }));
        thread.scrollTop = thread.scrollHeight;
    };
    $('#coachSend').addEventListener('click', () => send($('#coachInput').value.trim()));
    $('#coachInput').addEventListener('keydown', (e) => { if (e.key === 'Enter') send(e.target.value.trim()); });
    $$('.suggest button', el).forEach((b) => b.addEventListener('click', () => send(b.dataset.q)));
    $('#coachOfferAi')?.addEventListener('click', () => downloadOnDeviceModel());
}

/* Kick off an on-device model download with a progress sheet, then refresh the
   coach so it switches to the local backend. Offered only on eligible devices. */
async function downloadOnDeviceModel(modelId) {
    const info = store.localAI || await localAI.probe();
    const model = (info.models || []).find((m) => m.id === modelId) || info.models?.[0];
    if (!model) { toast('This device can’t run an on-device model.', 'var(--warn)'); return; }
    const host = $('#overlays');
    const paint = (pct, label) => {
        host.innerHTML = `<div class="sheet-dim"></div>
        <div class="sheet" role="dialog" aria-label="On-device AI">
            <div class="grab"></div>
            <h3>${esc(model.label)} · on-device</h3>
            <p style="color:var(--ink-2); font-size:var(--fs-sub); line-height:1.5; margin:2px 0 16px;">
                ${esc(model.download_mb)} MB · runs privately on your ${esc(TIER_LABEL[info.scan?.tier] || 'device').toLowerCase()}. Once installed, the coach answers offline.</p>
            <div class="splash-bar"><i style="width:${pct}%"></i></div>
            <div class="splash-status">${esc(label)}</div>
        </div>`;
    };
    paint(0, 'starting…');
    const res = await localAI.install(model.id, (pct, label) => paint(pct, `${label} ${pct}%`));
    store.localAI = await localAI.probe();
    host.innerHTML = '';
    if (res.ok) {
        sound.chime();
        toast('On-device AI ready — the coach now runs offline', 'var(--ok)');
        if (routeId() === 'coach' || routeId() === 'settings') render();
    } else {
        toast(res.error || 'Could not install the model', 'var(--err)');
    }
}
const msgHtml = (m) => m.role === 'user'
    ? `<div class="msg user">${esc(m.text)}</div>`
    : `<div class="msg ai"><div class="who">LIFELINE COACH</div>${esc(m.text)}</div>`;

/* ═══ VIEW: VAULT ══════════════════════════════════════════════════════════ */
let vaultTab = 'journal';
async function viewVault(el) {
    el.innerHTML = `
    ${offlineBanner()}
    <div class="page-head">
        <div class="eyebrow">E2EE · versioned · signed</div>
        <h1>The Vault.</h1>
        <div class="sub">Documents are encrypted before they leave the device; the server stores ciphertext it can never read.</div>
    </div>
    <div class="card">
        <div style="display:flex; align-items:center; gap:12px; margin-bottom:16px;">
            <div class="seg">
                <button data-t="journal" class="${vaultTab === 'journal' ? 'active' : ''}">Journal</button>
                <button data-t="labs" class="${vaultTab === 'labs' ? 'active' : ''}">Biomarkers</button>
            </div>
            <span style="margin-left:auto"></span>
            ${vaultTab === 'journal'
                ? '<button class="btn btn-pulse btn-sm" id="newEntryBtn">Sync journal entry</button>'
                : '<button class="btn btn-pulse btn-sm" id="uploadLabBtn">Upload lab panel</button>'}
        </div>
        <div id="vaultList"><div class="skeleton" style="height:64px"></div></div>
    </div>`;

    $$('.seg button', el).forEach((b) => b.addEventListener('click', () => { vaultTab = b.dataset.t; render(); }));
    $('#newEntryBtn')?.addEventListener('click', syncJournal);
    $('#uploadLabBtn')?.addEventListener('click', uploadLab);
    await renderVaultList();
}

async function renderVaultList() {
    const host = $('#vaultList');
    if (!host) return;
    const type = vaultTab === 'journal' ? 'health_journal' : 'lab_result';
    const res = await api.documentsByType(type);
    const docs = res.status === 200 ? (res.data.documents || []) : [];
    if (!docs.length) {
        host.innerHTML = `<div class="empty">${vaultTab === 'journal'
            ? 'No journal documents yet.<br>Sync one — it is AES-GCM ciphertext to the server, plaintext only to you.'
            : "No labs yet.<br>Upload a panel from your doctor — it's encrypted on-device, then plotted against its healthy range here."}</div>`;
        return;
    }
    docs.sort((a, b) => new Date(b.created_at) - new Date(a.created_at));
    host.innerHTML = docs.map((doc) => {
        const meta = store.labMeta[doc.document_id];
        const when = new Date(doc.created_at).toLocaleString();
        if (vaultTab === 'labs' && meta && store.insights) {
            const bm = engine.biomarker(store.insights, meta.key, meta.value);
            return `<div class="doc">
                <div class="doc-head"><span class="t"><span class="d" style="background:var(--cardio)"></span>${esc(meta.title)}</span><span class="m">${esc(meta.lab)} · ${esc(when)}</span></div>
                ${bm ? `<div class="bm">
                    <div class="bm-head"><span class="n">${esc(meta.name)}</span><span class="v" style="color:${bm.tone}">${bm.value} ${esc(bm.unit)}</span></div>
                    ${charts.rangeStrip(bm)}
                    <div class="bm-flag" style="color:${bm.tone}">${esc(bm.flag)}</div>
                </div>` : ''}
            </div>`;
        }
        return `<div class="doc">
            <div class="doc-head"><span class="t"><span class="d" style="background:var(--sleep)"></span>${vaultTab === 'journal' ? 'Journal entry' : 'Lab result'} <span class="tnum" style="font-weight:400;color:var(--ink-3)">v${doc.version_sequence}</span></span><span class="m">${esc(when)}</span></div>
            <div class="doc-body">Encrypted blob · opaque to the server · ${esc(String(doc.document_id).slice(0, 8))}…</div>
        </div>`;
    }).join('');
}

/* Encrypt on-device with AES-256-GCM, sign with the device's real P-256 key,
   then ship ciphertext. The server verifies the signature against the
   registered public key and stores bytes it can never decrypt. */
async function syncEncrypted(contents, documentType) {
    const id = crypto.randomUUID();
    const version = 1;
    const { blob, iv, tag } = await deviceCrypto.encrypt(contents);
    const signature = await deviceCrypto.signSync(id, version, blob, iv, tag);
    const res = await api.syncDelta({
        document_id: id, device_id: identity.deviceId, version_sequence: version,
        encrypted_blob: deviceCrypto.toB64(blob),
        initialization_vector: deviceCrypto.toB64(iv),
        auth_tag: deviceCrypto.toB64(tag),
        client_signature: signature,
        document_type: documentType,
    });
    return { id, res };
}

async function syncJournal() {
    const { res } = await syncEncrypted({ kind: 'journal', ts: new Date().toISOString() }, 'health_journal');
    if (res.status === 200) { toast('Journal entry encrypted, signed & synced'); await renderVaultList(); }
    else toast(res.data?.error?.message || `Sync failed (${res.status})`, 'var(--err)');
}

let labCursor = 0;
async function uploadLab() {
    const panel = engine.LAB_PANELS[labCursor++ % engine.LAB_PANELS.length];
    const { id, res } = await syncEncrypted({ kind: 'lab', ...panel }, 'lab_result');
    if (res.status === 200) {
        store.labMeta[id] = panel; saveLabMeta();
        toast(`${panel.title} uploaded — plotted vs reference range`);
        await renderVaultList();
    } else toast(res.data?.error?.message || `Upload failed (${res.status})`, 'var(--err)');
}

/* ═══ VIEW: SOURCES ════════════════════════════════════════════════════════ */
const PROVIDERS = [
    { id: 'apple_health', name: 'Apple Health', sub: 'HealthKit · read on-device only', color: 'var(--ink)', icon: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M12 21s-7-4.35-9.5-8.5C.5 8.5 2.5 5 6 5c2 0 3.5 1.2 6 3.5C14.5 6.2 16 5 18 5c3.5 0 5.5 3.5 3.5 7.5C19 16.65 12 21 12 21z"/></svg>' },
    { id: 'google_health', name: 'Google Health Connect', sub: 'Health Connect · read on-device only', color: 'var(--sleep)', icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9"><path d="M17.5 19a4.5 4.5 0 0 0 0-9 6 6 0 0 0-11.4-1.5A5 5 0 0 0 6.5 19h11z"/></svg>' },
    { id: 'whoop', name: 'Whoop', sub: 'OAuth2 · token encrypted at rest', color: 'var(--recovery)', icon: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9"><circle cx="12" cy="12" r="8"/><path d="M12 4v2m0 12v2" stroke-linecap="round"/></svg>' },
];

async function refreshSources() {
    const res = await api.integrations();
    store.connections = {};
    if (res.status === 200) (res.data.connections || []).forEach((c) => { store.connections[c.provider] = c; });
    if (whoopConnected()) {
        const m = await api.whoopMetrics();
        store.whoopMetrics = m.status === 200 ? m.data : null;
    } else store.whoopMetrics = null;
}

async function viewSources(el) {
    el.innerHTML = `
    ${offlineBanner()}
    <div class="page-head">
        <div class="eyebrow">Bring your own data</div>
        <h1>Sources.</h1>
        <div class="sub">Lifeline fuses every source into one readiness — without ever seeing the raw numbers.</div>
    </div>
    <div class="card">
        <div id="providerList">${PROVIDERS.map(() => '<div class="skeleton" style="height:58px; margin-bottom:10px"></div>').join('')}</div>
        <div id="whoopPanel"></div>
        <div class="note" style="margin-top:16px;"><b>Apple &amp; Google Health</b> are on-device SDKs — connecting just records your consent. <b>Whoop</b> is a cloud API: its refresh token is sealed with ChaCha20-Poly1305 and never returned to any client.</div>
    </div>`;
    await refreshSources();
    paintProviders();
}

function paintProviders() {
    const host = $('#providerList');
    if (!host) return;
    host.innerHTML = PROVIDERS.map((p) => {
        const conn = store.connections[p.id];
        const on = conn?.status === 'connected';
        return `<div class="rowline">
            <div class="mark" style="background:${p.color}">${p.icon}</div>
            <div class="grow"><h4>${p.name}</h4><p>${on ? `connected${conn.last_synced_at ? ' · synced ' + new Date(conn.last_synced_at).toLocaleTimeString() : ''}` : p.sub}</p></div>
            <button class="btn btn-sm ${on ? 'btn-ghost' : 'btn-pulse'}" data-p="${p.id}">${on ? 'Disconnect' : 'Connect'}</button>
        </div>`;
    }).join('');
    $$('#providerList [data-p]').forEach((b) => b.addEventListener('click', () => toggleProvider(b.dataset.p, b)));

    const w = $('#whoopPanel');
    if (w) {
        w.innerHTML = whoopConnected() && store.whoopMetrics ? `
        <div class="tiles" style="margin-top:14px;">
            <div class="tile"><div class="v tnum">${store.whoopMetrics.recovery_score ?? '—'}<small>%</small></div><div class="l">recovery</div></div>
            <div class="tile"><div class="v tnum">${store.whoopMetrics.strain ?? '—'}</div><div class="l">strain</div></div>
            <div class="tile"><div class="v tnum">${store.whoopMetrics.hrv_ms ?? '—'}<small>ms</small></div><div class="l">hrv</div></div>
        </div>` : '';
    }
}

async function toggleProvider(id, btn) {
    btn.disabled = true;
    const on = store.connections[id]?.status === 'connected';
    if (on) {
        const r = await api.disconnectProvider(id);
        if (r.status === 200) toast(`${id.replace('_', ' ')} disconnected`, 'var(--warn)');
    } else if (id === 'whoop') {
        const a = await api.whoopAuthorize();
        if (a.status === 200 && a.data?.authorize_url) {
            const url = new URL(a.data.authorize_url, location.origin);
            const cb = await api.whoopCallback(url.search);
            if (cb.status === 200) toast('Whoop connected via OAuth2');
            else toast(cb.data?.error?.message || 'Whoop connect failed', 'var(--err)');
        } else toast(a.data?.error?.message || 'Whoop authorize failed', 'var(--err)');
    } else {
        const r = await api.connectProvider(id);
        if (r.status === 200) toast(`${id.replace('_', ' ')} connected`);
        else toast(r.data?.error?.message || 'Connect failed', 'var(--err)');
    }
    await refreshSources();
    paintProviders();
    if (routeId() === 'portrait') render();
}

/* ═══ VIEW: PLANS ══════════════════════════════════════════════════════════ */
async function refreshBillingState() {
    const s = await api.subscription();
    store.sub = s.status === 200 ? s.data : null;
    if (store.sub?.tier === 'elite') {
        const b = await api.betaFeatures();
        store.beta = b.status === 200 ? b.data : null;
    } else store.beta = null;
}

async function viewPlans(el) {
    el.innerHTML = `${offlineBanner()}
    <div class="page-head">
        <div class="eyebrow">Stripe · no card data touches Lifeline</div>
        <h1>Plans.</h1>
        <div class="sub">Start free. Upgrade when the competition gets serious.</div>
    </div>
    <div id="plansHost"><div class="skeleton" style="height:280px"></div></div>`;
    await refreshBillingState();
    paintPlans();
}

function paintPlans() {
    const host = $('#plansHost');
    if (!host || !store.billing) return;
    const cur = store.sub?.tier || 'free';
    const order = ['free', 'pro', 'elite'];
    const live = store.billing.live;

    host.innerHTML = `
    <div class="plans">
        ${store.billing.tiers.map((t) => {
            const isCur = t.tier === cur;
            const isUp = order.indexOf(t.tier) > order.indexOf(cur);
            const price = t.price_monthly_usd === 0 ? 'Free' : `$${t.price_monthly_usd.toFixed(2)}<small> /mo</small>`;
            return `<div class="plan ${isCur ? 'current' : ''} ${t.tier === 'elite' && !isCur ? 'featured' : ''}">
                ${t.tier === 'elite' ? '<span class="flag">most complete</span>' : ''}
                ${t.tier === 'pro' && !isCur ? '<span class="flag pop">most popular</span>' : ''}
                <h3>${esc(t.name)}</h3>
                <div class="price tnum">${price}</div>
                <div class="tag">${esc(t.tagline)}</div>
                <ul>${t.features.map((f) => `<li>${I.check}${esc(f)}</li>`).join('')}</ul>
                ${isCur ? '<div class="cur">✓ Your current plan</div>'
                    : isUp ? (IN_STORE_SHELL
                        ? `<button class="btn btn-pulse btn-block" data-iap="${t.tier}">Subscribe</button>`
                        : `<button class="btn btn-pulse btn-block" data-up="${t.tier}">Upgrade</button>`)
                    : '<div class="below">Included in your plan</div>'}
            </div>`;
        }).join('')}
    </div>
    ${store.beta ? `<div class="beta-box">
        <h4>Beta channel · Elite</h4>
        ${(store.beta.builds || []).map((b) => `<div class="beta-row"><span>${esc(b.notes)}</span><span class="ver">${esc(b.version)}</span></div>`).join('')}
    </div>` : ''}
    <div style="display:flex; gap:10px; margin-top:16px; align-items:center; flex-wrap:wrap;">
        ${cur !== 'free' && !IN_STORE_SHELL ? '<button class="btn btn-ghost" id="portalBtn">Manage subscription</button>' : ''}
        <span style="font-size:var(--fs-micro); color:var(--ink-3);">${IN_STORE_SHELL
            ? 'Purchases are handled by the App Store / Google Play. Manage or cancel in your store subscription settings.'
            : live ? 'Live billing via Stripe Checkout.' : 'Stripe test mode — upgrades are simulated server-side, no card charged.'}</span>
    </div>
    ${IN_STORE_SHELL ? '' : `
    <div class="card donate-card">
        <div class="card-title">Fuel the mission ☕</div>
        <div class="card-sub">Donations unlock nothing — they just keep the free tier free, for everyone, forever.</div>
        <div class="donate-row">
            ${(store.billing?.donate?.presets_usd || [3, 5, 10]).map((usd) => `<button class="btn btn-ghost donate-amt ${usd === 5 ? 'sel' : ''}" data-usd="${usd}">$${usd}</button>`).join('')}
            <button class="btn btn-pulse" id="donateBtn">Donate <span class="tnum" id="donateLabel">$5</span> ❤️</button>
        </div>
    </div>`}`;

    $$('#plansHost [data-up]').forEach((b) => b.addEventListener('click', async () => {
        b.disabled = true;
        const res = await api.checkout(b.dataset.up);
        if (res.status === 200) {
            if (res.data.simulated) {
                sound.chime(); confetti();
                toast(`Upgraded to ${b.dataset.up} — simulated checkout`);
                await refreshBillingState();
                paintPlans();
            } else if (res.data.checkout_url) {
                window.open(res.data.checkout_url, '_blank');
                toast('Stripe Checkout opened in a new tab');
            }
        } else toast(res.data?.error?.message || `Checkout failed (${res.status})`, 'var(--err)');
    }));

    // Native purchases (store shells): StoreKit / Play Billing via the
    // window.LifelineIAP bridge, then server-side receipt redemption. The
    // same entitlement machinery as Stripe — only the money moves elsewhere.
    $$('#plansHost [data-iap]').forEach((b) => b.addEventListener('click', async () => {
        const tier = b.dataset.iap;
        const bridge = window.LifelineIAP;
        if (!bridge?.purchase) {
            toast('In-app purchase is available in the store build', 'var(--warn-deep)');
            return;
        }
        b.disabled = true;
        try {
            const r = await bridge.purchase(tier); // → { platform, receipt }
            const res = await api.storeReceipt(r.platform, tier, r.receipt);
            if (res.status === 200) {
                sound.chime(); confetti();
                toast(`Welcome to ${cap(tier)} — verified with the store`);
                await refreshBillingState();
                paintPlans();
            } else {
                toast(res.data?.error?.message || 'Purchase could not be verified', 'var(--err)');
            }
        } catch {
            toast('Purchase cancelled', 'var(--warn)');
        }
        b.disabled = false;
    }));

    // Donate: preset selection (rule of three) + one warm button.
    let donateUsd = 5;
    $$('#plansHost .donate-amt').forEach((b) => b.addEventListener('click', () => {
        donateUsd = Number(b.dataset.usd);
        $$('#plansHost .donate-amt').forEach((x) => x.classList.toggle('sel', x === b));
        const lbl = $('#donateLabel');
        if (lbl) lbl.textContent = `$${donateUsd}`;
    }));
    $('#donateBtn')?.addEventListener('click', async () => {
        const preset = store.billing?.donate?.url;
        if (preset) { window.open(preset, '_blank'); return; }
        const res = await api.donate(donateUsd * 100);
        if (res.status === 200) {
            if (res.data.simulated) {
                sound.coin(); confetti();
                toast('❤️ Thank you — every coffee keeps the free tier free');
            } else if (res.data.checkout_url) {
                window.open(res.data.checkout_url, '_blank');
            }
        } else toast(res.data?.error?.message || 'Donation failed', 'var(--err)');
    });
    $('#portalBtn')?.addEventListener('click', async () => {
        const res = await api.portal();
        if (res.status === 200 && res.data?.portal_url) {
            if (res.data.simulated) toast('Billing portal (simulated) — manage/cancel lives here in production');
            else window.open(res.data.portal_url, '_blank');
        } else toast(res.data?.error?.message || 'Portal unavailable', 'var(--err)');
    });
}

/* ═══ VIEW: SETTINGS ═══════════════════════════════════════════════════════ */
async function viewSettings(el) {
    const theme = localStorage.getItem('lifeline.theme') || 'auto';
    const acct = account.current;
    const ai = store.localAI || (store.localAI = await localAI.probe());
    const health = await api.health();
    el.innerHTML = `${offlineBanner()}
    <div class="page-head">
        <div class="eyebrow">Device · privacy · appearance</div>
        <h1>Settings.</h1>
    </div>
    <div class="grid">
        <div class="card col-6">
            <div class="card-title">Account &amp; identity</div>
            <div class="card-sub">your account secures access; your device holds the keys — health data stays here</div>
            <div class="kv"><span class="k">Account</span><span class="v">${esc(acct?.email || (acct?.auth_method ? cap(acct.auth_method) + ' sign-in' : '—'))}</span></div>
            <div class="kv"><span class="k">Sign-in method</span><span class="v">${esc(acct?.auth_method ? cap(acct.auth_method) : 'device only')}</span></div>
            <div class="kv"><span class="k">Device ID</span><span class="v">${esc(identity.deviceId.slice(0, 13))}…</span></div>
            <div class="kv"><span class="k">Session</span><span class="v">${status().authed ? 'active' : 'none'}</span></div>
            <div class="kv"><span class="k">Plan</span><span class="v">${esc(store.sub?.tier || 'free')}</span></div>
            <div style="margin-top:14px; display:flex; gap:9px; flex-wrap:wrap;">
                ${acct ? '<button class="btn btn-ghost btn-sm" id="signOutBtn">Sign out</button>' : ''}
                <button class="btn btn-ghost btn-sm" id="resetBtn">Reset identity</button>
                <button class="btn btn-ghost btn-sm" id="replayBtn">Replay onboarding</button>
            </div>
            <div class="danger-zone">
                <div class="dz-copy">
                    <b>Delete account</b>
                    <span>Permanently erases your account and all data on our servers — vault, scores, subscription. This can’t be undone.</span>
                </div>
                <button class="btn btn-danger btn-sm" id="deleteAcctBtn">Delete account</button>
            </div>
        </div>
        <div class="card col-6">
            <div class="card-title">Engine</div>
            <div class="card-sub">live from the backend this page is served by</div>
            <div class="kv"><span class="k">Backend</span><span class="v">${health ? (health.status || 'ok') : 'offline'}</span></div>
            <div class="kv"><span class="k">Billing mode</span><span class="v">${store.billing?.live ? 'live (Stripe)' : 'test / simulated'}</span></div>
            <div class="kv"><span class="k">Insights rulebook</span><span class="v">v${esc(store.insights?.version || '—')}</span></div>
            <div class="kv"><span class="k">Season</span><span class="v">${esc(store.game?.season?.current || '—')}</span></div>
        </div>
        <div class="card col-6">
            <div class="card-title">Your focus</div>
            <div class="card-sub">shapes your Today view and biases the coach — <b>Auto</b> follows your weakest signal</div>
            <div class="seg seg-wrap" id="focusSeg">
                ${['auto', 'sleep', 'heart', 'activity', 'longevity'].map((f) => `<button data-focus="${f}" class="${userFocus() === f ? 'active' : ''}">${FOCI[f].label}</button>`).join('')}
            </div>
            ${notify.supported() ? `
            <div class="kv" style="margin-top:14px;">
                <span class="k">Daily check-in<br><small style="color:var(--ink-3); font-weight:500; font-size:var(--fs-micro)">a once-a-day AI note about your day</small></span>
                <span class="switch ${notify.enabled() && notify.permission() === 'granted' ? 'on' : ''}" id="notifySwitch" role="switch" aria-checked="${notify.enabled() && notify.permission() === 'granted'}" tabindex="0"></span>
            </div>` : ''}
        </div>
        <div class="card col-6">
            <div class="card-title">Appearance &amp; feel</div>
            <div class="card-sub">the light theme is designed, not inverted</div>
            <div class="seg" id="themeSeg">
                ${['auto', 'light', 'dark'].map((t) => `<button data-th="${t}" class="${theme === t ? 'active' : ''}">${cap(t)}</button>`).join('')}
            </div>
            <div class="kv" style="margin-top:10px;"><span class="k">Sounds</span><span class="switch ${sound.enabled ? 'on' : ''}" id="soundSwitch" role="switch" aria-checked="${sound.enabled}" tabindex="0"></span></div>
        </div>
        <div class="card col-6">
            <div class="card-title">Privacy model</div>
            <div class="kv"><span class="k">Raw biometrics</span><span class="v">never leave device</span></div>
            <div class="kv"><span class="k">Vault documents</span><span class="v">E2EE ciphertext</span></div>
            <div class="kv"><span class="k">Arena shares</span><span class="v">one opaque integer</span></div>
            <div class="kv"><span class="k">Coach</span><span class="v">${localAI.ready() ? 'on-device · offline' : 'identity-stripped proxy'}</span></div>
        </div>
        ${onDeviceAiCard(ai)}
        <div class="card col-12" style="border:1px solid var(--line);">
            <div class="card-title">About &amp; disclaimer</div>
            <p style="font-size:var(--fs-micro); color:var(--ink-3); line-height:1.5; margin:2px 0 8px;">
                Lifeline is for informational and wellness purposes and is <b>not a medical device</b>.
                It does not diagnose, treat, cure, or prevent any condition, and its scores and guidance
                are not medical advice. Always consult a qualified clinician for medical decisions.
            </p>
            <a class="kv" href="/privacy" target="_blank" rel="noopener" style="text-decoration:none;">
                <span class="k">Privacy policy</span><span class="v">how zero-knowledge works →</span>
            </a>
        </div>
    </div>`;

    $('#signOutBtn')?.addEventListener('click', () => {
        if (confirm('Sign out of this account? Your encrypted data stays on this device.')) {
            account.signOut();
            location.reload();
        }
    });
    $('#deleteAcctBtn')?.addEventListener('click', confirmDeleteAccount);
    $('#resetBtn').addEventListener('click', () => {
        if (confirm('Reset this device identity? Your handle, plan, and vault links are tied to it.')) identity.reset();
    });
    $('#replayBtn').addEventListener('click', () => { localStorage.removeItem('lifeline.onboarded'); location.reload(); });
    $$('#themeSeg [data-th]').forEach((b) => b.addEventListener('click', () => {
        localStorage.setItem('lifeline.theme', b.dataset.th);
        applyTheme();
        $$('#themeSeg button').forEach((x) => x.classList.toggle('active', x === b));
    }));
    $$('#focusSeg [data-focus]').forEach((b) => b.addEventListener('click', () => {
        setUserFocus(b.dataset.focus);
        $$('#focusSeg button').forEach((x) => x.classList.toggle('active', x === b));
        toast(`Focus set to ${FOCI[b.dataset.focus].label} — your Today view will lead with it`);
    }));
    $('#notifySwitch')?.addEventListener('click', async (e) => {
        const el = e.currentTarget;
        if (el.classList.contains('on')) {
            notify.disable();
            el.classList.remove('on');
            el.setAttribute('aria-checked', 'false');
            toast('Daily check-in off');
            return;
        }
        const ok = await notify.enable();
        el.classList.toggle('on', ok);
        el.setAttribute('aria-checked', String(ok));
        if (ok) {
            toast('Daily check-in on — your first note is on its way');
            // Fire today's note now as a welcome (respects the once-a-day gate).
            const text = await generateDailyAnecdote();
            await notify.show(store.insights?.anecdote?.notification_title || 'Your Lifeline is ready', text);
        } else {
            toast('Notifications are blocked — enable them in your device settings', 'var(--warn)');
        }
    });
    $('#soundSwitch')?.addEventListener('click', (e) => {
        sound.setEnabled(!sound.enabled);
        e.currentTarget.classList.toggle('on', sound.enabled);
        e.currentTarget.setAttribute('aria-checked', String(sound.enabled));
    });
    $('#installAiBtn')?.addEventListener('click', (e) => downloadOnDeviceModel(e.currentTarget.dataset.model));
    $('#removeAiBtn')?.addEventListener('click', async () => {
        if (confirm('Remove the on-device model? The coach will use the private cloud proxy again.')) {
            await localAI.remove();
            store.localAI = await localAI.probe();
            render();
        }
    });
}

/* The "Private AI" card: shows what this device scored, and — on eligible
   premium devices — lets the user download a Gemma model to run the coach
   entirely offline. On devices that can't, it explains why, honestly. */
function onDeviceAiCard(ai) {
    if (!ai) return '';
    const tier = ai.scan?.tier || 'entry';
    const ready = localAI.ready();
    const ramTxt = ai.scan?.ramGb ? `${ai.scan.ramGb}${ai.scan.ramExact ? '' : '+'} GB` : 'unknown';
    const backendTxt = ai.backend === 'native' ? 'native ML runtime'
        : ai.backend === 'webgpu' ? 'WebGPU'
        : ai.scan?.backends?.length ? ai.scan.backends.join(', ') : 'none detected';
    const rec = (ai.models || [])[0];

    let action;
    if (ready) {
        action = `<div class="ai-ready">${I.check}<span>Running on-device — the coach works offline.</span></div>
            <button class="btn btn-ghost btn-sm" id="removeAiBtn" style="margin-top:12px;">Remove model</button>`;
    } else if (ai.eligible && rec) {
        action = `<button class="btn btn-pulse btn-sm btn-block" id="installAiBtn" data-model="${esc(rec.id)}" style="margin-top:12px;">
            Download ${esc(rec.label)} · ${esc(rec.download_mb)} MB</button>
            <p class="ai-note">Runs the coach privately on this device. After install it works with no internet at all.</p>`;
    } else {
        action = `<p class="ai-note">This device isn’t powerful enough to run a private model well, so the coach uses the identity-stripping cloud proxy. On-device AI is offered on premium phones with ≥4 GB RAM and a modern accelerator.</p>`;
    }

    return `<div class="card col-12 ai-card">
        <div class="card-title">Private on-device AI ${ready ? '<span class="pill live">active</span>' : ai.eligible ? '<span class="pill">available</span>' : ''}</div>
        <div class="card-sub">run Lifeline’s coach as a local model — no network, nothing leaves your phone</div>
        <div class="ai-scan">
            <div class="tile"><div class="v">${esc(TIER_LABEL[tier] || tier)}</div><div class="l">device class</div></div>
            <div class="tile"><div class="v tnum">${esc(ramTxt)}</div><div class="l">memory</div></div>
            <div class="tile"><div class="v tnum">${ai.scan?.cores ?? '—'}</div><div class="l">cores</div></div>
            <div class="tile"><div class="v" style="font-size:var(--fs-sub)">${esc(backendTxt)}</div><div class="l">inference</div></div>
        </div>
        ${action}
    </div>`;
}

/* Permanent account deletion (App Store 5.1.1(v)). A clear, two-tap destructive
   confirmation sheet, then a server-side erase + full local wipe, then back to
   the sign-in gate. */
function confirmDeleteAccount() {
    const host = $('#overlays');
    host.innerHTML = `
        <div class="sheet-dim" id="dzDim"></div>
        <div class="sheet" role="dialog" aria-label="Delete account" aria-describedby="dzText">
            <div class="grab"></div>
            <h3>Delete your account?</h3>
            <p id="dzText" style="color:var(--ink-2); font-size:var(--fs-sub); line-height:1.55; margin:2px 0 18px;">
                This permanently erases your account and <b>everything</b> tied to it on our
                servers — your encrypted vault, Arena scores, streak, and subscription record.
                It cannot be undone. Your on-device data is wiped too.</p>
            <div style="display:flex; flex-direction:column; gap:9px;">
                <button class="btn btn-danger btn-block" id="dzConfirm">Delete forever</button>
                <button class="btn btn-ghost btn-block" id="dzCancel">Keep my account</button>
            </div>
        </div>`;
    const close = () => { host.innerHTML = ''; };
    $('#dzDim').addEventListener('click', close);
    $('#dzCancel').addEventListener('click', close);
    $('#dzConfirm').addEventListener('click', async () => {
        const btn = $('#dzConfirm');
        btn.disabled = true; btn.textContent = 'Deleting…';
        const res = await account.deleteAccount();
        if (res.status === 200) {
            // Local state already wiped by deleteAccount(); reload into the gate.
            location.reload();
        } else {
            close();
            toast(res.data?.error?.message || `Couldn’t delete account (${res.status})`, 'var(--err)');
        }
    });
}

function applyTheme() {
    const t = localStorage.getItem('lifeline.theme') || 'auto';
    if (t === 'auto') document.documentElement.removeAttribute('data-theme');
    else document.documentElement.setAttribute('data-theme', t);
}

/* ═══ ROUTER ═══════════════════════════════════════════════════════════════ */
const VIEWS = {
    portrait: viewPortrait,
    arena: viewArena,
    coach: viewCoach,
    vault: viewVault,
    sources: viewSources,
    plans: viewPlans,
    settings: viewSettings,
};

async function render() {
    applyConductor();   // recolor + reorder the shell for today's mode
    paintTabbar();      // reflect any reorder in the tab bar
    const id = VIEWS[routeId()] ? routeId() : 'portrait';
    setActiveNav(id);
    const el = $('#view');
    if (!el) return;
    await VIEWS[id](el);
    el.scrollTop = 0;
}
window.addEventListener('hashchange', render);

/* ═══ ONBOARDING + BOOT ════════════════════════════════════════════════════ */
function cap(s) { return s ? s[0].toUpperCase() + s.slice(1) : s; }

function onboarding() {
    return new Promise((resolve) => {
        const host = $('#overlays');
        let step = 0;
        const steps = [
            () => `
                <div class="gate-logo">${I.logo}</div>
                <h1>Your health,<br>competing quietly.</h1>
                <p class="lede">Lifeline turns your body's signals into one daily score, a biological age, and a place on a global ladder — without your data ever leaving this device.</p>`,
            () => `
                <div class="gate-logo">${I.logo}</div>
                <h1>Zero-knowledge,<br>not a promise. A design.</h1>
                <div class="zk-points">
                    <div class="pt">${I.lock}<p><b>Everything is computed here.</b> The server publishes rules; your device does the math.</p></div>
                    <div class="pt">${I.eye}<p><b>The server is blind.</b> Vault documents arrive as ciphertext. The Arena sees one opaque integer.</p></div>
                    <div class="pt">${I.device}<p><b>Hardware-attested.</b> Your device holds the keys. Your account only unlocks the door — it never holds your health data.</p></div>
                </div>`,
            () => `
                <div class="gate-logo">${I.logo}</div>
                <h1>Ready.</h1>
                <p class="lede">Your device key is being attested and your portrait drawn. This takes a moment.</p>
                <div class="splash-bar"><i id="bootBar"></i></div>
                <div class="splash-status" id="bootStatus">initializing…</div>`,
        ];
        const paint = () => {
            host.innerHTML = `<div class="gate"><div class="gate-inner">
                ${steps[step]()}
                ${step < 2 ? `<div class="actions"><button class="btn btn-pulse btn-block" id="nextBtn">${step === 0 ? 'Begin' : 'Attest my device'}</button></div>
                <div class="dots">${steps.map((_, i) => `<i class="${i <= step ? 'on' : ''}"></i>`).join('')}</div>
                <button class="skip" id="skipBtn">Skip intro</button>` : `<div class="dots">${steps.map((_, i) => `<i class="${i <= step ? 'on' : ''}"></i>`).join('')}</div>`}
            </div></div>`;
            $('#nextBtn')?.addEventListener('click', () => { step += 1; paint(); if (step === 2) resolve(); });
            $('#skipBtn')?.addEventListener('click', () => { step = 2; paint(); resolve(); });
        };
        paint();
    });
}

async function bootSequence(withGate) {
    const setP = (pct, txt) => {
        const bar = $('#bootBar'), st = $('#bootStatus');
        if (bar) bar.style.width = pct + '%';
        if (st) st.textContent = txt;
    };
    setP(12, 'connecting to engine…');
    await connect();
    setP(34, 'attesting device…');
    const [ins, game, billing] = await Promise.all([api.insightsConfig(), api.gameConfig(), api.billingConfig()]);
    if (ins.status === 200) store.insights = ins.data;
    if (game.status === 200) store.game = game.data;
    if (billing.status === 200) store.billing = billing.data;
    setP(66, 'loading rulebooks…');
    // Real sensor data on the native shells (HealthKit / Health Connect); the
    // deterministic browser simulation otherwise.
    store.signals = await refreshSignals();
    // Scan the device and check what on-device AI (if any) it can run.
    try { store.localAI = await localAI.probe(); } catch { store.localAI = null; }
    if (status().authed) {
        await Promise.all([refreshArena(), refreshBillingState(), refreshSources()]);
    }
    setP(90, 'drawing your portrait…');
    await new Promise((r) => setTimeout(r, withGate ? 420 : 0));
    setP(100, 'ready');
    await new Promise((r) => setTimeout(r, withGate ? 260 : 0));
}

/* ═══ SIGN IN / SIGN UP GATE ═══════════════════════════════════════════════
   Stands between onboarding and the app. Email + password (server-side
   PBKDF2), plus Continue with Apple / Google. Resolves once a real account
   session is live. The device must be reachable first — offline, the gate
   explains and lets the user retry. */
function authGate() {
    return new Promise((resolve) => {
        const host = $('#overlays');
        let mode = 'signin'; // 'signin' | 'signup'
        const APPLE = '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M16.4 12.9c0-2.3 1.9-3.4 2-3.5-1.1-1.6-2.8-1.8-3.4-1.8-1.4-.1-2.8.9-3.5.9s-1.8-.9-3-.8c-1.5 0-3 .9-3.8 2.3-1.6 2.8-.4 7 1.2 9.3.8 1.1 1.7 2.4 2.9 2.3 1.2 0 1.6-.7 3-.7s1.8.7 3 .7 2-1.1 2.8-2.2c.9-1.3 1.2-2.5 1.3-2.6-.1 0-2.5-1-2.5-3.8zM14.2 6.3c.6-.8 1.1-1.9.9-3-.9 0-2.1.6-2.8 1.4-.6.7-1.1 1.8-1 2.9 1 .1 2.1-.5 2.9-1.3z"/></svg>';
        const GOOGLE = '<svg viewBox="0 0 24 24"><path fill="#4285F4" d="M23 12.3c0-.8-.1-1.6-.2-2.3H12v4.5h6.2a5.3 5.3 0 0 1-2.3 3.5v2.9h3.7c2.2-2 3.4-5 3.4-8.6z"/><path fill="#34A853" d="M12 24c3.1 0 5.7-1 7.6-2.8l-3.7-2.9c-1 .7-2.3 1.1-3.9 1.1-3 0-5.5-2-6.4-4.7H1.8v3A11.5 11.5 0 0 0 12 24z"/><path fill="#FBBC05" d="M5.6 14.7a6.9 6.9 0 0 1 0-4.4v-3H1.8a11.5 11.5 0 0 0 0 10.4l3.8-3z"/><path fill="#EA4335" d="M12 4.8c1.7 0 3.2.6 4.4 1.7l3.3-3.3A11.5 11.5 0 0 0 12 0 11.5 11.5 0 0 0 1.8 6.3l3.8 3C6.5 6.7 9 4.8 12 4.8z"/></svg>';

        const emailRe = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
        const setErr = (m) => { const e = $('#authErr'); if (e) e.textContent = m || ''; };
        const busy = (on, label) => {
            $$('.gate button').forEach((b) => { b.disabled = on; });
            const pb = $('#authPrimary');
            if (pb && label) pb.textContent = on ? 'One moment…' : label;
        };

        const finish = () => { host.innerHTML = ''; resolve(); };

        const handle = async (fn, label) => {
            setErr('');
            busy(true, label);
            const res = await fn();
            busy(false, label);
            if (res.status === 200 && res.data?.token) {
                sound.chime();
                finish();
                return;
            }
            const msg = res.data?.error?.message
                || (res.status === 0 ? 'Can’t reach the engine — check your connection.' : 'Something went wrong. Try again.');
            setErr(msg);
        };

        const paint = () => {
            const signup = mode === 'signup';
            host.innerHTML = `<div class="gate"><div class="gate-inner">
                <div class="gate-logo">${I.logo}</div>
                <h1>${signup ? 'Create your<br>Lifeline.' : 'Welcome<br>back.'}</h1>
                <p class="lede">${signup
                    ? 'One private account to carry your longevity across every device — your health data still never leaves this one.'
                    : 'Sign in to pick up your portrait, streak, and rank.'}</p>
                <div class="seg auth-seg" role="tablist">
                    <button data-mode="signin" class="${!signup ? 'active' : ''}">Sign in</button>
                    <button data-mode="signup" class="${signup ? 'active' : ''}">Create account</button>
                </div>
                <div class="auth-form">
                    <input class="field" id="authEmail" type="email" inputmode="email" autocomplete="email"
                        autocapitalize="off" spellcheck="false" placeholder="Email address">
                    <input class="field" id="authPass" type="password"
                        autocomplete="${signup ? 'new-password' : 'current-password'}"
                        placeholder="${signup ? 'Create a password (8+ characters)' : 'Password'}">
                    <button class="btn btn-pulse btn-block" id="authPrimary">${signup ? 'Create account' : 'Sign in'}</button>
                    <div class="auth-err" id="authErr" role="alert"></div>
                </div>
                <div class="auth-or"><span>or continue with</span></div>
                <div class="auth-social">
                    <button class="btn social-btn social-apple" id="appleBtn">${APPLE}<span>Apple</span></button>
                    <button class="btn social-btn social-google" id="googleBtn">${GOOGLE}<span>Google</span></button>
                </div>
                <p class="auth-fine">${I.lock}<span>Zero-knowledge by design — your account secures access, never your health data.</span></p>
            </div></div>`;

            $$('.auth-seg button').forEach((b) => b.addEventListener('click', () => {
                mode = b.dataset.mode; paint();
            }));

            $('#authPrimary').addEventListener('click', () => {
                const email = $('#authEmail').value.trim();
                const pass = $('#authPass').value;
                if (!emailRe.test(email)) { setErr('Enter a valid email address.'); return; }
                if (pass.length < 8) { setErr('Password must be at least 8 characters.'); return; }
                const label = signup ? 'Create account' : 'Sign in';
                handle(() => (signup ? account.register(email, pass) : account.login(email, pass)), label);
            });

            const social = (provider) => {
                const email = $('#authEmail').value.trim() || '';
                if (email && !emailRe.test(email)) { setErr('That email looks off — clear it or fix it.'); return; }
                // On the native shells, run the real Sign in with Apple / Google
                // flow and pass the provider's id-token to the backend to verify;
                // on the web, fall back to the simulated token.
                const nativeFn = window.LifelineSignIn?.[provider];
                handle(async () => {
                    let idToken;
                    if (nativeFn) {
                        try { idToken = await nativeFn(); }
                        catch { return { status: 0, data: { error: { message: `${provider} sign-in was cancelled.` } } }; }
                    }
                    return account.social(provider, email, idToken);
                });
            };
            $('#appleBtn').addEventListener('click', () => social('apple'));
            $('#googleBtn').addEventListener('click', () => social('google'));

            $('#authEmail').focus();
        };
        paint();
    });
}

async function main() {
    applyTheme();
    armGlobalSounds();
    // Register the offline service worker first thing — independent of sign-in —
    // so the app shell + rulebooks cache even before the user reaches the app.
    registerServiceWorker();
    renderFrame();
    const first = !localStorage.getItem('lifeline.onboarded');
    if (first) {
        await onboarding();
        await bootSequence(true);
        localStorage.setItem('lifeline.onboarded', '1');
    } else {
        await bootSequence(false);
    }
    // Sign-in / sign-up gate: require a real account before entering the app.
    // Show it whenever the engine is reachable and we don't already hold a
    // working session for a remembered account — i.e. a brand-new user, or a
    // returning user whose device session couldn't be re-established silently
    // (as in production, where the browser can't mint a dev session). Offline,
    // we let the user in read-only rather than trap them behind a login they
    // can't complete.
    if (status().online && (!account.current || !status().authed)) {
        await authGate();
        // Load the per-account server truth now that we're authed.
        if (status().authed) {
            await Promise.all([refreshArena(), refreshBillingState(), refreshSources()]);
        }
    }
    $('#overlays').innerHTML = '';
    if (first) {
        toast(status().authed ? `Signed in${account.current?.email ? ' as ' + account.current.email : ''}` : 'Running offline', status().authed ? 'var(--ok)' : 'var(--warn)');
    }
    keepAlive();
    await render();
    registerServiceWorker();
    // Once-a-day AI note → OS notification, if the user has opted in.
    maybeSendDailyNotification();
    // Refresh signals + portrait at midnight rollover (and pick up fresh native
    // health data through the day).
    setInterval(async () => {
        const fresh = await refreshSignals();
        if (JSON.stringify(fresh) !== JSON.stringify(store.signals)) {
            store.signals = fresh;
            store.anecdote = null; // a new day → a fresh note
            if (routeId() === 'portrait') render();
            maybeSendDailyNotification();
        }
    }, 60_000);
}

/* Register the offline service worker. It precaches the shell + rulebooks so
   that — once an on-device model is installed — the app works with no network.
   Best-effort and non-blocking: a registration failure never affects the app.
   Requires a secure context (https or localhost), which the SW spec enforces. */
function registerServiceWorker() {
    if (!('serviceWorker' in navigator)) return;
    if (!window.isSecureContext) return;
    const reg = () => navigator.serviceWorker.register('/sw.js').catch((e) => {
        console.warn('Service worker registration failed:', e);
    });
    // main() is async, so the window 'load' event may already have fired by now.
    if (document.readyState === 'complete') reg();
    else window.addEventListener('load', reg, { once: true });
}

main();
