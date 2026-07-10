/* Lifeline on-device insights engine.
   The server ships *rules* (/insights/config): band tables, weights,
   reference ranges, chronotype windows. This module applies them to health
   signals that never leave the device. In the web app the signals are a
   deterministic daily simulation (a browser has no HealthKit); the native
   iOS app feeds real sensor data through the same math. */

/* Deterministic per-day variation so the portrait breathes day to day
   without a backend for raw data. */
function daySeed() {
    const d = new Date();
    const key = d.getFullYear() * 10000 + (d.getMonth() + 1) * 100 + d.getDate();
    let x = key ^ 0x9e3779b9;
    x = Math.imul(x ^ (x >>> 16), 0x45d9f3b);
    x = Math.imul(x ^ (x >>> 16), 0x45d9f3b);
    return ((x ^ (x >>> 16)) >>> 0) / 0xffffffff;
}

export function todaySignals() {
    const r = daySeed();
    const wave = (amp) => (r - 0.5) * 2 * amp;
    return {
        chrono_age: 34,
        resting_heart_rate: Math.round(63 + wave(6)),
        hrv_ms: Math.round(64 + wave(14)),
        sleep_hours: Math.round((7.3 + wave(0.8)) * 10) / 10,
        daily_steps: Math.round(8900 + wave(2600)),
        sleep_performance: Math.round(86 + wave(9)),
        prior_strain: Math.round((11 + wave(4)) * 10) / 10,
        sleep_midpoint: 3.6,
    };
}

function bandLookup(bands, v) {
    for (const b of bands) if (v <= b.max) return b.years;
    return bands[bands.length - 1].years;
}

export function lifelineAge(cfg, s) {
    const sig = cfg.biological_age.signals;
    let off = 0;
    off += bandLookup(sig.resting_heart_rate.bands, s.resting_heart_rate);
    off += bandLookup(sig.hrv_ms.bands, s.hrv_ms);
    off += bandLookup(sig.sleep_hours.bands, s.sleep_hours);
    off += bandLookup(sig.daily_steps.bands, s.daily_steps);
    const clamp = cfg.biological_age.clamp_years;
    off = Math.max(-clamp, Math.min(clamp, off));
    return { age: Math.round((s.chrono_age + off) * 10) / 10, offset: Math.round(off * 10) / 10 };
}

function scoreComponent(c, v) {
    let t = (v - c.poor_at) / (c.good_at - c.poor_at);
    if (c.invert) t = (c.poor_at - v) / (c.poor_at - c.good_at);
    return Math.max(0, Math.min(1, t));
}

/* Readiness fused over whichever sources are connected. `prior_strain` only
   participates when Whoop is linked — weights renormalize on the rest. */
export function readiness(cfg, s, { whoop = false } = {}) {
    const comp = cfg.readiness.components;
    const vals = {
        hrv: s.hrv_ms,
        resting_heart_rate: s.resting_heart_rate,
        sleep_performance: s.sleep_performance,
        prior_strain: whoop ? s.prior_strain : null,
    };
    let sum = 0, wsum = 0, worst = null, best = null;
    for (const [k, c] of Object.entries(comp)) {
        if (vals[k] == null) continue;
        const sc = scoreComponent(c, vals[k]);
        sum += sc * c.weight; wsum += c.weight;
        if (!worst || sc < worst.sc) worst = { k, sc };
        if (!best || sc > best.sc) best = { k, sc };
    }
    const score = wsum ? Math.round((sum / wsum) * 100) : 0;
    const label = (cfg.readiness.labels.find((l) => score >= l.min) || {}).text || '—';
    const names = { hrv: 'HRV', resting_heart_rate: 'resting heart rate', sleep_performance: 'sleep', prior_strain: "yesterday's strain" };
    const driver = worst && worst.sc < 0.6
        ? `held back by ${names[worst.k]}`
        : `${names[best?.k] || 'recovery'} is your strongest signal`;
    return { score, label, driver };
}

/* The one number that ever leaves the device: readiness fused with the
   healthspan offset. Opaque — no biometric can be recovered from it. */
export function vitality(cfg, s, opts) {
    const r = readiness(cfg, s, opts);
    const la = lifelineAge(cfg, s);
    const healthspan = Math.max(0, Math.min(100, Math.round(50 - la.offset * 6)));
    return Math.max(0, Math.min(100, Math.round(r.score * 0.55 + healthspan * 0.45)));
}

export function correlations(cfg) {
    return Object.entries(cfg.correlation.habits)
        .map(([k, v]) => ({ key: k, name: k.replace(/_/g, ' '), prior: v.prior }))
        .sort((a, b) => b.prior - a.prior);
}

export function chronotype(cfg, s) {
    const mid = s.sleep_midpoint;
    const type = mid < 3 ? 'lark' : mid > 4.5 ? 'owl' : 'neutral';
    return { type, windows: cfg.circadian.chronotypes[type] };
}

/* Per-signal deviation from ideal, for the signal bars: 0 = at ideal band,
   1 = worst band. Uses the same band tables that drive Lifeline Age. */
export function signalDeviations(cfg, s) {
    const sig = cfg.biological_age.signals;
    const spec = [
        { key: 'resting_heart_rate', name: 'Resting HR', pigment: 'cardio', value: s.resting_heart_rate, unit: 'bpm', bands: sig.resting_heart_rate.bands },
        { key: 'hrv_ms', name: 'HRV', pigment: 'recovery', value: s.hrv_ms, unit: 'ms', bands: sig.hrv_ms.bands },
        { key: 'sleep_hours', name: 'Sleep', pigment: 'sleep', value: s.sleep_hours, unit: 'h', bands: sig.sleep_hours.bands },
        { key: 'daily_steps', name: 'Steps', pigment: 'activity', value: s.daily_steps, unit: '', bands: sig.daily_steps.bands },
    ];
    return spec.map((it) => {
        const years = bandLookup(it.bands, it.value);
        const worst = Math.max(...it.bands.map((b) => Math.abs(b.years)));
        const goodness = 1 - Math.max(0, years + worst) / (2 * worst); // -worst→1, +worst→0
        return { ...it, years, goodness: Math.max(0.06, Math.min(1, goodness)) };
    });
}

/* Evaluate one uploaded biomarker value against the server's reference
   ranges. Returns render-ready bands on a 0..scaleMax axis plus a flag. */
export function biomarker(cfg, key, value) {
    const r = cfg.biomarkers[key];
    if (!r) return null;
    let scaleMax, bands, flag, tone;
    const A = 'var(--ok)', W = 'var(--warn)', E = 'var(--err)';
    if (key === 'ldl_cholesterol') {
        scaleMax = 200;
        bands = [[0, r.optimal_max, A], [r.optimal_max, r.high_min, W], [r.high_min, scaleMax, E]];
        [flag, tone] = value <= r.optimal_max ? ['in optimal range', A] : value < r.high_min ? ['borderline — keep an eye on it', W] : ['high — discuss with your doctor', E];
    } else if (key === 'vitamin_d') {
        scaleMax = 80;
        bands = [[0, r.deficient_max, E], [r.deficient_max, r.optimal_min, W], [r.optimal_min, scaleMax, A]];
        [flag, tone] = value >= r.optimal_min ? ['optimal', A] : value > r.deficient_max ? ['below optimal — sun or supplement', W] : ['deficient', E];
    } else if (key === 'hba1c' || key === 'fasting_glucose') {
        scaleMax = key === 'hba1c' ? 8 : 140;
        bands = [[0, r.optimal_max, A], [r.optimal_max, r.diabetic_min, W], [r.diabetic_min, scaleMax, E]];
        [flag, tone] = value <= r.optimal_max ? ['optimal', A] : value < r.diabetic_min ? ['watch — prediabetic band', W] : ['elevated', E];
    } else if (key === 'tsh') {
        scaleMax = 6;
        bands = [[0, r.optimal_min, W], [r.optimal_min, r.optimal_max, A], [r.optimal_max, scaleMax, W]];
        const inRange = value >= r.optimal_min && value <= r.optimal_max;
        [flag, tone] = inRange ? ['in optimal range', A] : ['outside optimal — recheck', W];
    } else if (key === 'hdl_cholesterol') {
        scaleMax = 100;
        bands = [[0, r.low_max, E], [r.low_max, r.optimal_min, W], [r.optimal_min, scaleMax, A]];
        [flag, tone] = value >= r.optimal_min ? ['optimal', A] : value > r.low_max ? ['fair — raise with activity', W] : ['low', E];
    } else {
        return null;
    }
    return { unit: r.unit, scaleMax, bands, flag, tone, value };
}

/* The lab panels the demo Vault can "upload", with plausible values. */
export const LAB_PANELS = [
    { title: 'Lipid Panel', lab: 'Quest Diagnostics', key: 'ldl_cholesterol', name: 'LDL cholesterol', value: 118 },
    { title: 'HDL Check', lab: 'LabCorp', key: 'hdl_cholesterol', name: 'HDL cholesterol', value: 64 },
    { title: 'Metabolic Panel', lab: 'City Health Partners', key: 'fasting_glucose', name: 'Fasting glucose', value: 92 },
    { title: 'HbA1c', lab: 'Mercy Medical Lab', key: 'hba1c', name: 'HbA1c', value: 5.3 },
    { title: 'Thyroid Panel', lab: 'Quest Diagnostics', key: 'tsh', name: 'TSH', value: 2.1 },
    { title: 'Vitamin D, 25-OH', lab: 'LabCorp', key: 'vitamin_d', name: 'Vitamin D', value: 33 },
];

/* ── Game math mirrors (server is authoritative; these render instantly) ── */
export function leagueFor(gameCfg, score) {
    let out = gameCfg.leagues[0];
    for (const l of gameCfg.leagues) if (score >= l.min_score) out = l;
    return out;
}
export const levelFor = (xp) => Math.floor(Math.sqrt(Math.max(0, xp) / 90)) + 1;
export const xpForLevel = (lvl) => (lvl <= 1 ? 0 : Math.ceil((lvl - 1) * (lvl - 1) * 90));
