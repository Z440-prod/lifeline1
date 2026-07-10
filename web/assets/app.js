/* Lifeline — application shell.
   Hash-routed SPA over the Antigravity backend. Every feature talks to the
   real API: sessions come from /auth/dev-session (development), scores and
   ranks from /game/*, entitlements from /billing/*, documents from /sync/*,
   providers from /integrations/*. */

import { api, connect, keepAlive, identity, onConnection, status, deviceCrypto } from './api.js';
import * as engine from './engine.js';
import * as charts from './charts.js';

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
};
const saveLabMeta = () => localStorage.setItem('lifeline.labs', JSON.stringify(store.labMeta));

const whoopConnected = () => store.connections.whoop?.status === 'connected';
const vitalityNow = () => store.insights ? engine.vitality(store.insights, store.signals, { whoop: whoopConnected() }) : 0;

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
    eye: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9"><path d="M2 12s3.5-6.5 10-6.5S22 12 22 12s-3.5 6.5-10 6.5S2 12 2 12z"/><circle cx="12" cy="12" r="2.6"/><path d="M4 4l16 16" stroke-linecap="round"/></svg>',
    device: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9"><rect x="7" y="2.5" width="10" height="19" rx="2.8"/><path d="M11 18.5h2" stroke-linecap="round"/></svg>',
    logo: '<svg viewBox="0 0 32 32"><rect width="32" height="32" rx="8" fill="var(--surface)" stroke="var(--hairline)"/><path d="M5 18h5.5l2.8-7.5 3.8 11 2.8-5.5H27" fill="none" stroke="var(--pulse)" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round"/></svg>',
};
const PIGMENT = { cardio: 'var(--cardio)', sleep: 'var(--sleep)', activity: 'var(--activity)', energy: 'var(--energy)', recovery: 'var(--recovery)' };

/* ── Routes ───────────────────────────────────────────────────────────────── */
const ROUTES = [
    { id: 'portrait', label: 'Portrait', icon: I.portrait },
    { id: 'arena', label: 'Arena', icon: I.arena },
    { id: 'coach', label: 'Coach', icon: I.coach },
    { id: 'vault', label: 'Vault', icon: I.vault },
    { id: 'sources', label: 'Sources', icon: I.sources },
    { id: 'plans', label: 'Plans', icon: I.plans },
    { id: 'settings', label: 'Settings', icon: I.settings },
];
const routeId = () => (location.hash.replace(/^#\/?/, '') || 'portrait').split('?')[0];

/* ── Frame ────────────────────────────────────────────────────────────────── */
function renderFrame() {
    $('#app').innerHTML = `
    <div class="frame">
        <aside class="sidebar">
            <div class="brand">${I.logo}<div class="word">Lifeline<small>LONGEVITY ENGINE</small></div></div>
            <nav class="nav">
                ${ROUTES.map((r) => `<button class="nav-item" data-nav="${r.id}">${r.icon}${r.label}</button>`).join('')}
            </nav>
            <div class="conn" id="connBadge"><span class="dot"></span><span class="t">connecting…</span></div>
        </aside>
        <main class="main"><div class="content" id="view"></div></main>
        <nav class="tabbar">
            ${ROUTES.slice(0, 5).map((r) => `<button class="tab" data-nav="${r.id}">${r.icon}<span>${r.label}</span></button>`).join('')}
            <button class="tab" data-nav="settings">${I.settings}<span>More</span></button>
        </nav>
    </div>`;
    $$('[data-nav]').forEach((b) => b.addEventListener('click', () => { location.hash = `#/${b.dataset.nav}`; }));
    onConnection((s) => {
        const el = $('#connBadge');
        if (!el) return;
        el.classList.toggle('online', s.online && s.authed);
        $('.t', el).textContent = s.online ? (s.authed ? 'connected · attested session' : 'connected · no session') : 'backend offline';
    });
}

function setActiveNav(id) {
    $$('[data-nav]').forEach((b) => b.classList.toggle('active', b.dataset.nav === id));
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

    el.innerHTML = `
    ${offlineBanner()}
    <div class="page-head">
        <div class="eyebrow">${esc(dateStr)}</div>
        <h1>${hello}.</h1>
        <div class="sub">Computed on this device from your signals — the server never sees a heartbeat.</div>
    </div>
    <div class="grid">
        <div class="card hero col-12">
            <div class="vitality-hero">
                <div class="vitality-num">
                    <span class="n tnum">${v}</span>
                    <span class="cap">today's vitality — the one number that ever leaves this device</span>
                </div>
                <div class="vitality-trace">${charts.pulseTrace({ vitality: v, rhr: s.resting_heart_rate })}</div>
                <div class="vitality-side">
                    ${lg ? `<span class="chip"><span class="d" style="background:${charts.LEAGUE_COLORS[lg.id]}"></span>${esc(lg.name)} league</span>` : ''}
                    <button class="btn btn-pulse btn-sm" id="logScoreBtn">Log today's score</button>
                </div>
            </div>
        </div>

        <div class="card col-4">
            <div class="card-title">Readiness</div>
            <div class="card-sub">fused from every connected source</div>
            <div style="display:flex; justify-content:center;">${charts.ringGauge({ value: r.score, label: r.label })}</div>
            <p style="text-align:center; font-size:var(--fs-small); color:var(--ink-2); margin-top:10px;">${esc(r.driver)}</p>
        </div>

        <div class="card col-4">
            <div class="card-title">Lifeline Age</div>
            <div class="card-sub">transparent additive model — inspect it in Settings</div>
            <div class="tiles">
                <div class="tile"><div class="v tnum">${la.age}</div><div class="l">biological</div>
                    <div class="delta" style="color:${la.offset <= 0 ? 'var(--ok)' : 'var(--warn)'}">${la.offset <= 0 ? '▼' : '▲'} ${Math.abs(la.offset)} yrs vs calendar</div></div>
                <div class="tile"><div class="v tnum">${s.chrono_age}</div><div class="l">calendar</div></div>
            </div>
        </div>

        <div class="card col-4">
            <div class="card-title">Circadian window</div>
            <div class="card-sub">${esc(chrono.type)} chronotype · shifted to your sleep midpoint</div>
            ${charts.circadianTrack(chrono.windows, 420)}
        </div>

        <div class="card col-7">
            <div class="card-title">Signals vs ideal <span class="hint">band tables from /insights/config</span></div>
            <div class="card-sub">bar length = how close each signal sits to its optimal band</div>
            <div class="signal-rows">
                ${devs.map((d) => `
                <div class="signal-row" title="${esc(d.name)}: ${d.value}${d.unit} → ${d.years <= 0 ? '' : '+'}${d.years} yrs on Lifeline Age">
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

    $('#logScoreBtn')?.addEventListener('click', () => submitScoreFlow());
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
    if (!store.profile && !handle) {
        location.hash = '#/arena';
        toast('Claim a handle first to join the Arena', 'var(--warn)');
        return;
    }
    const res = await api.submitScore(v, handle);
    if (res.status === 200) {
        store.profile = res.data;
        toast(`Logged ${v} — ${res.data.league} league, #${res.data.rank} worldwide`);
        await refreshArena();
        if (routeId() === 'arena' || routeId() === 'portrait') render();
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
                    <div class="standing">as <b>${esc(p.handle)}</b> · level ${p.level} · ${p.streak_days}-day streak</div>
                    <div class="standing">rank <b>#${p.rank}</b> of ${p.population} · top ${Math.max(1, Math.round(100 - p.percentile) || 1)}%</div>
                    <div class="xp-wrap">
                        <div class="meter" style="height:8px"><i style="width:${Math.min(100, Math.round((xpInto / xpSpan) * 100))}%; background:linear-gradient(90deg, var(--energy), var(--pulse))"></i></div>
                        <div class="xp-caption"><span>level ${p.level}</span><span>${Math.max(0, xpSpan - xpInto)} XP → ${p.level + 1}</span></div>
                    </div>` : `
                    <div class="league">Unranked</div>
                    <div class="standing">Claim a handle and log your first vitality score to enter the global ladder.</div>
                    <div style="display:flex; gap:8px; margin-top:12px; max-width:340px;">
                        <input class="field" id="handleInput" maxlength="20" placeholder="pick a handle (3–20, a–z 0–9 _)" spellcheck="false">
                        <button class="btn btn-pulse" id="joinBtn">Join</button>
                    </div>`}
                </div>
            </div>
            ${p ? `<div style="margin-top:16px; display:flex; gap:9px;">
                <button class="btn btn-pulse" id="logBtn">Log today's score · ${vitalityNow()}</button>
            </div>` : ''}
        </div>

        <div class="card col-6">
            <div class="card-title">Today's stats</div>
            <div class="card-sub">what your next submission carries</div>
            <div class="tiles">
                <div class="tile"><div class="v tnum">${vitalityNow()}</div><div class="l">vitality</div></div>
                <div class="tile"><div class="v tnum">${p ? p.streak_days : 0}</div><div class="l">day streak</div></div>
                <div class="tile"><div class="v tnum">${p ? Number(p.season_xp).toLocaleString() : 0}</div><div class="l">season xp</div></div>
                <div class="tile"><div class="v tnum">${p ? p.best_vitality_score : '—'}</div><div class="l">best score</div></div>
            </div>
            <div class="note" style="margin-top:14px;"><b>One submission a day counts.</b> XP = 40 + 2·score + 5·streak (streak capped at 30). Leagues are score bands: ${g ? g.leagues.map((l) => `${l.name} ${l.min_score}+`).join(' · ') : '—'}.</div>
        </div>

        <div class="card col-12">
            <div class="card-title">Global leaderboard <span class="hint">live from /game/leaderboard</span></div>
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
    el.innerHTML = `
    ${offlineBanner()}
    <div class="page-head">
        <div class="eyebrow">Clinical-first · zero retention</div>
        <h1>Coach.</h1>
        <div class="sub">Every message routes through the privacy proxy: stripped of identity, never stored, answered with your on-device context.</div>
    </div>
    <div class="card">
        <div class="coach-thread" id="thread">
            <div class="msg sys">end-to-end private — the proxy strips your identity before the model sees a word</div>
            ${store.coachLog.map(msgHtml).join('')}
        </div>
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
        let reply;
        try {
            const ch = await api.challenge();
            const res = await api.aiProxy(text, ch.data?.challenge || 'token');
            reply = res.status === 200
                ? (res.data?.content?.[0]?.text || res.data?.content || 'Understood.')
                : `The proxy answered ${res.status}: ${res.data?.error?.message || 'unavailable'}.`;
        } catch { reply = 'Could not reach the backend.'; }
        document.getElementById(tid)?.remove();
        store.coachLog.push({ role: 'ai', text: reply });
        thread.insertAdjacentHTML('beforeend', msgHtml({ role: 'ai', text: reply }));
        thread.scrollTop = thread.scrollHeight;
    };
    $('#coachSend').addEventListener('click', () => send($('#coachInput').value.trim()));
    $('#coachInput').addEventListener('keydown', (e) => { if (e.key === 'Enter') send(e.target.value.trim()); });
    $$('.suggest button', el).forEach((b) => b.addEventListener('click', () => send(b.dataset.q)));
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
                <h3>${esc(t.name)}</h3>
                <div class="price tnum">${price}</div>
                <div class="tag">${esc(t.tagline)}</div>
                <ul>${t.features.map((f) => `<li>${I.check}${esc(f)}</li>`).join('')}</ul>
                ${isCur ? '<div class="cur">✓ Your current plan</div>'
                    : isUp ? `<button class="btn btn-pulse btn-block" data-up="${t.tier}">Upgrade</button>`
                    : '<div class="below">Included in your plan</div>'}
            </div>`;
        }).join('')}
    </div>
    ${store.beta ? `<div class="beta-box">
        <h4>Beta channel · Elite</h4>
        ${(store.beta.builds || []).map((b) => `<div class="beta-row"><span>${esc(b.notes)}</span><span class="ver">${esc(b.version)}</span></div>`).join('')}
    </div>` : ''}
    <div style="display:flex; gap:10px; margin-top:16px; align-items:center; flex-wrap:wrap;">
        ${cur !== 'free' ? '<button class="btn btn-ghost" id="portalBtn">Manage subscription</button>' : ''}
        <span style="font-size:var(--fs-micro); color:var(--ink-3);">${live ? 'Live billing via Stripe Checkout.' : 'Stripe test mode — upgrades are simulated server-side, no card charged.'}</span>
    </div>`;

    $$('#plansHost [data-up]').forEach((b) => b.addEventListener('click', async () => {
        b.disabled = true;
        const res = await api.checkout(b.dataset.up);
        if (res.status === 200) {
            if (res.data.simulated) {
                toast(`Upgraded to ${b.dataset.up} — simulated checkout`);
                await refreshBillingState();
                paintPlans();
            } else if (res.data.checkout_url) {
                window.open(res.data.checkout_url, '_blank');
                toast('Stripe Checkout opened in a new tab');
            }
        } else toast(res.data?.error?.message || `Checkout failed (${res.status})`, 'var(--err)');
    }));
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
    const health = await api.health();
    el.innerHTML = `${offlineBanner()}
    <div class="page-head">
        <div class="eyebrow">Device · privacy · appearance</div>
        <h1>Settings.</h1>
    </div>
    <div class="grid">
        <div class="card col-6">
            <div class="card-title">Identity</div>
            <div class="card-sub">your pseudonymous device identity — there is no account, by design</div>
            <div class="kv"><span class="k">Device ID</span><span class="v">${esc(identity.deviceId.slice(0, 13))}…</span></div>
            <div class="kv"><span class="k">Session</span><span class="v">${status().authed ? 'attested · active' : 'none'}</span></div>
            <div class="kv"><span class="k">Handle</span><span class="v">${esc(store.profile?.handle || '—')}</span></div>
            <div class="kv"><span class="k">Plan</span><span class="v">${esc(store.sub?.tier || 'free')}</span></div>
            <div style="margin-top:14px; display:flex; gap:9px;">
                <button class="btn btn-ghost btn-sm" id="resetBtn">Reset identity</button>
                <button class="btn btn-ghost btn-sm" id="replayBtn">Replay onboarding</button>
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
            <div class="card-title">Appearance</div>
            <div class="card-sub">the light theme is designed, not inverted</div>
            <div class="seg" id="themeSeg">
                ${['auto', 'light', 'dark'].map((t) => `<button data-th="${t}" class="${theme === t ? 'active' : ''}">${cap(t)}</button>`).join('')}
            </div>
        </div>
        <div class="card col-6">
            <div class="card-title">Privacy model</div>
            <div class="kv"><span class="k">Raw biometrics</span><span class="v">never leave device</span></div>
            <div class="kv"><span class="k">Vault documents</span><span class="v">E2EE ciphertext</span></div>
            <div class="kv"><span class="k">Arena shares</span><span class="v">one opaque integer</span></div>
            <div class="kv"><span class="k">Coach proxy</span><span class="v">identity-stripped</span></div>
        </div>
    </div>`;

    $('#resetBtn').addEventListener('click', () => {
        if (confirm('Reset this device identity? Your handle, plan, and vault links are tied to it.')) identity.reset();
    });
    $('#replayBtn').addEventListener('click', () => { localStorage.removeItem('lifeline.onboarded'); location.reload(); });
    $$('#themeSeg [data-th]').forEach((b) => b.addEventListener('click', () => {
        localStorage.setItem('lifeline.theme', b.dataset.th);
        applyTheme();
        $$('#themeSeg button').forEach((x) => x.classList.toggle('active', x === b));
    }));
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
                    <div class="pt">${I.device}<p><b>Hardware-attested.</b> On iOS, Apple App Attest proves it's really your device — no passwords, no accounts.</p></div>
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
    if (status().authed) {
        await Promise.all([refreshArena(), refreshBillingState(), refreshSources()]);
    }
    setP(90, 'drawing your portrait…');
    await new Promise((r) => setTimeout(r, withGate ? 420 : 0));
    setP(100, 'ready');
    await new Promise((r) => setTimeout(r, withGate ? 260 : 0));
}

async function main() {
    applyTheme();
    renderFrame();
    const first = !localStorage.getItem('lifeline.onboarded');
    if (first) {
        await onboarding();
        await bootSequence(true);
        localStorage.setItem('lifeline.onboarded', '1');
        $('#overlays').innerHTML = '';
        toast(status().authed ? 'Device attested — session active' : 'Running offline', status().authed ? 'var(--ok)' : 'var(--warn)');
    } else {
        await bootSequence(false);
    }
    keepAlive();
    await render();
    // Refresh signals + portrait at midnight rollover.
    setInterval(() => {
        const fresh = engine.todaySignals();
        if (JSON.stringify(fresh) !== JSON.stringify(store.signals)) {
            store.signals = fresh;
            if (routeId() === 'portrait') render();
        }
    }, 60_000);
}

main();
