/* Lifeline admin dashboard.
   Fetches aggregate stats from /api/v1/admin/stats with the admin token. The
   token lives only in sessionStorage (cleared on tab close / sign-out) and is
   never persisted. All data shown is aggregate + pseudonymous — no PII. */

const $ = (s) => document.querySelector(s);
const esc = (t) => { const d = document.createElement('div'); d.textContent = String(t ?? ''); return d.innerHTML; };
const KEY = 'lifeline.admin.token';

const login = $('#login');
const dash = $('#dash');

function fmt(n) { return (n ?? 0).toLocaleString(); }
function uptime(sec) {
    sec = Math.max(0, sec | 0);
    const d = Math.floor(sec / 86400), h = Math.floor((sec % 86400) / 3600), m = Math.floor((sec % 3600) / 60);
    return d ? `${d}d ${h}h` : h ? `${h}h ${m}m` : `${m}m`;
}

async function loadStats(token) {
    const res = await fetch('/api/v1/admin/stats', { headers: { Authorization: `Bearer ${token}` } });
    if (res.status === 200) return { ok: true, data: await res.json() };
    let msg = 'Access denied.';
    if (res.status === 403) msg = 'The admin dashboard is disabled on this deployment (no admin token configured).';
    else if (res.status === 401) msg = 'Invalid admin token.';
    else msg = `Unexpected error (${res.status}).`;
    return { ok: false, status: res.status, msg };
}

function render(d) {
    const s = d.system || {}, u = d.users || {}, v = d.vault || {}, a = d.arena || {}, b = d.billing || {}, ai = d.ai || {};
    $('#sys').innerHTML = `v<b>${esc(s.version)}</b> · ${esc(s.environment)} · ${esc(s.database)} · up <b>${uptime(s.uptime_seconds)}</b>`;

    const maxLeague = Math.max(1, ...(a.leagues || []).map((l) => l.count));
    const leagueBars = (a.leagues || []).length
        ? (a.leagues).map((l) => `<div class="bar-row"><span class="lb">${esc(l.league)}</span><span class="bar"><i style="width:${Math.round((l.count / maxLeague) * 100)}%"></i></span><span class="n">${fmt(l.count)}</span></div>`).join('')
        : '<div class="muted">No ranked players yet.</div>';

    const top = (a.top || []).length
        ? (a.top).map((p, i) => `<div class="row"><span class="rank">${i + 1}</span><span class="name">${esc(p.handle)}</span><span class="lg">${esc(p.league)}</span><span class="score">${fmt(p.score)}</span></div>`).join('')
        : '<div class="muted">No scores logged yet.</div>';

    const aiPct = ai.global_daily_budget ? Math.round((ai.coach_messages_today / ai.global_daily_budget) * 100) : 0;

    $('#content').innerHTML = `
    <div class="grid">
        <div class="stat"><div class="v">${fmt(u.accounts)}</div><div class="l">Accounts</div></div>
        <div class="stat"><div class="v">${fmt(u.devices)}</div><div class="l">Devices</div></div>
        <div class="stat"><div class="v">${fmt(v.documents)}</div><div class="l">Vault docs</div><div class="sub">${fmt(v.versions)} versions</div></div>
        <div class="stat"><div class="v">${fmt(a.ranked_players)}</div><div class="l">Ranked players</div></div>
    </div>
    <div class="grid">
        <div class="stat"><div class="v" style="color:var(--pro)">${fmt(b.pro)}</div><div class="l">Pro subs</div></div>
        <div class="stat"><div class="v" style="color:var(--elite)">${fmt(b.elite)}</div><div class="l">Elite subs</div></div>
        <div class="stat"><div class="v">${fmt(b.free_devices)}</div><div class="l">Free devices</div></div>
        <div class="stat"><div class="v">$${fmt(b.estimated_mrr_usd)}</div><div class="l">Est. MRR</div><div class="sub">at list prices</div></div>
    </div>
    <div class="card">
        <h3>AI coach usage · today</h3>
        <div class="bar-row"><span class="lb">Global</span><span class="bar"><i style="width:${Math.min(100, aiPct)}%; background:${aiPct > 85 ? 'var(--warn)' : 'var(--tint)'}"></i></span><span class="n">${fmt(ai.coach_messages_today)}</span></div>
        <div class="muted">${fmt(ai.coach_messages_today)} of ${fmt(ai.global_daily_budget)} daily budget (${aiPct}%)</div>
    </div>
    <div class="card">
        <h3>League distribution</h3>
        ${leagueBars}
    </div>
    <div class="card">
        <h3>Top players</h3>
        ${top}
    </div>
    <div class="muted" style="margin-top:16px;">Aggregate stats only — no health data or personal information is shown or stored here. Generated ${esc(new Date(d.generated_at).toLocaleString())}.</div>`;
}

async function show(token) {
    const r = await loadStats(token);
    if (!r.ok) {
        sessionStorage.removeItem(KEY);
        login.classList.remove('hidden');
        dash.classList.add('hidden');
        $('#loginErr').textContent = r.msg;
        return;
    }
    sessionStorage.setItem(KEY, token);
    login.classList.add('hidden');
    dash.classList.remove('hidden');
    render(r.data);
}

$('#signin').addEventListener('click', () => {
    const t = $('#token').value.trim();
    if (!t) { $('#loginErr').textContent = 'Enter the admin token.'; return; }
    $('#loginErr').textContent = '';
    show(t);
});
$('#token').addEventListener('keydown', (e) => { if (e.key === 'Enter') $('#signin').click(); });
$('#refresh').addEventListener('click', () => { const t = sessionStorage.getItem(KEY); if (t) show(t); });
$('#signout').addEventListener('click', () => {
    sessionStorage.removeItem(KEY);
    dash.classList.add('hidden');
    login.classList.remove('hidden');
    $('#token').value = '';
});

// Auto-load if a token is already in this session.
const existing = sessionStorage.getItem(KEY);
if (existing) show(existing);
