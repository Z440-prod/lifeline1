/* Lifeline chart renderers — hand-rolled SVG, themed via CSS variables so
   every mark re-colors correctly in both themes. Marks stay thin, direct
   labels are selective, and hover carries native tooltips via <title>. */

const NS = 'http://www.w3.org/2000/svg';

/* ── Pulse trace (hero) ─────────────────────────────────────────────────────
   The signature mark: one ECG-style lifeline whose beat spacing tightens
   with resting HR and whose baseline noise falls as vitality rises. */
export function pulseTrace({ vitality, rhr }, width = 560, height = 84) {
    const mid = height * 0.58;
    const beats = Math.max(3, Math.round(width / (620 - rhr * 4)));
    const period = width / beats;
    const calm = vitality / 100; // higher vitality → cleaner baseline
    let d = `M 0 ${mid.toFixed(1)}`;
    for (let b = 0; b < beats; b++) {
        const x0 = b * period;
        const jitter = (1 - calm) * 3;
        const w = (f) => (x0 + period * f).toFixed(1);
        d += ` L ${w(0.30)} ${(mid + jitter).toFixed(1)}`
            + ` L ${w(0.38)} ${(mid - height * 0.10).toFixed(1)}`
            + ` L ${w(0.44)} ${(mid + height * 0.06).toFixed(1)}`
            + ` L ${w(0.50)} ${(mid - height * 0.46).toFixed(1)}`
            + ` L ${w(0.56)} ${(mid + height * 0.28).toFixed(1)}`
            + ` L ${w(0.62)} ${(mid - jitter).toFixed(1)}`
            + ` L ${w(0.78)} ${(mid - height * 0.07).toFixed(1)}`
            + ` L ${w(0.92)} ${mid.toFixed(1)}`;
    }
    return `<svg viewBox="0 0 ${width} ${height}" preserveAspectRatio="none" role="img" aria-label="Vitality pulse trace">
        <title>Vitality ${vitality} · resting HR ${rhr} bpm</title>
        <line x1="0" y1="${mid}" x2="${width}" y2="${mid}" stroke="var(--hairline)" stroke-width="1"/>
        <path d="${d}" fill="none" stroke="var(--pulse)" stroke-width="2" stroke-linejoin="round" stroke-linecap="round" vector-effect="non-scaling-stroke"/>
    </svg>`;
}

/* ── Ring gauge (readiness) ───────────────────────────────────────────────── */
export function ringGauge({ value, label, color = 'var(--recovery)' }, size = 132) {
    const r = size / 2 - 10;
    const C = 2 * Math.PI * r;
    const off = C * (1 - value / 100);
    return `<svg viewBox="0 0 ${size} ${size}" width="${size}" height="${size}" role="img" aria-label="${label} ${value} of 100">
        <title>${label}: ${value} / 100</title>
        <circle cx="${size / 2}" cy="${size / 2}" r="${r}" fill="none" stroke="var(--hairline-2)" stroke-width="8"/>
        <circle cx="${size / 2}" cy="${size / 2}" r="${r}" fill="none" stroke="${color}" stroke-width="8"
            stroke-linecap="round" stroke-dasharray="${C.toFixed(1)}" stroke-dashoffset="${off.toFixed(1)}"
            transform="rotate(-90 ${size / 2} ${size / 2})" style="transition: stroke-dashoffset 1s var(--ease)"/>
        <text x="50%" y="49%" text-anchor="middle" dominant-baseline="central"
            font-family="var(--font-display)" font-weight="700" font-size="${size * 0.26}" fill="var(--ink)">${value}</text>
        <text x="50%" y="66%" text-anchor="middle" font-family="var(--font-mono)" font-size="${size * 0.082}"
            letter-spacing="0.12em" fill="var(--ink-3)">${label.toUpperCase()}</text>
    </svg>`;
}

/* ── Circadian day-track ──────────────────────────────────────────────────── */
export function circadianTrack(windows, width = 520) {
    const H = 74, pad = 14, y = 34;
    const toMin = (hhmm) => { const [h, m] = hhmm.split(':').map(Number); return h * 60 + m; };
    const x = (min) => pad + (min / 1440) * (width - 2 * pad);
    const marks = [
        { t: windows.peak_focus, label: 'peak focus', color: 'var(--activity)' },
        { t: windows.last_caffeine, label: 'last caffeine', color: 'var(--energy)' },
        { t: windows.wind_down, label: 'wind down', color: 'var(--recovery)' },
    ];
    let svg = `<svg viewBox="0 0 ${width} ${H}" role="img" aria-label="Circadian timing windows">`;
    svg += `<line x1="${pad}" y1="${y}" x2="${width - pad}" y2="${y}" stroke="var(--hairline)" stroke-width="2"/>`;
    for (const h of [0, 6, 12, 18, 24]) {
        const hx = x(h * 60);
        svg += `<line x1="${hx}" y1="${y - 4}" x2="${hx}" y2="${y + 4}" stroke="var(--hairline)"/>`
            + `<text x="${hx}" y="${y + 22}" text-anchor="middle" font-family="var(--font-mono)" font-size="9.5" fill="var(--ink-3)">${String(h).padStart(2, '0')}</text>`;
    }
    for (const m of marks) {
        const mx = x(toMin(m.t));
        svg += `<g><title>${m.label} · ${m.t}</title>`
            + `<circle cx="${mx.toFixed(1)}" cy="${y}" r="6.5" fill="${m.color}" stroke="var(--surface)" stroke-width="2"/>`
            + `<text x="${mx.toFixed(1)}" y="${y - 13}" text-anchor="middle" font-family="var(--font-ui)" font-size="10" font-weight="600" fill="var(--ink-2)">${m.label}</text></g>`;
    }
    return svg + '</svg>';
}

/* ── League emblem (arena) ────────────────────────────────────────────────── */
export const LEAGUE_COLORS = {
    bronze: '#a9713c', silver: '#8a94a6', gold: '#bf8814',
    platinum: '#3f7f8c', diamond: '#4a86ec', apex: '#e05a52',
};
export function leagueEmblem(leagueId, level, size = 92) {
    const c = LEAGUE_COLORS[leagueId] || '#8a94a6';
    return `<svg viewBox="0 0 92 92" width="${size}" height="${size}" role="img" aria-label="${leagueId} league, level ${level}">
        <title>${leagueId} league · level ${level}</title>
        <path d="M46 5 82 25 82 60 46 87 10 60 10 25Z" fill="${c}" fill-opacity="0.14" stroke="${c}" stroke-width="2.2"/>
        <path d="M46 15 73 30 73 55 46 75 19 55 19 30Z" fill="none" stroke="${c}" stroke-width="1.1" stroke-dasharray="2.5 3.5"/>
        <path d="M27 47h9l5-13 7 20 5-10h12" fill="none" stroke="${c}" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>
        <text x="46" y="66" text-anchor="middle" font-family="var(--font-mono)" font-size="13" font-weight="600" fill="var(--ink)">${level}</text>
    </svg>`;
}

/* ── Biomarker range strip ────────────────────────────────────────────────── */
export function rangeStrip(bm) {
    const pct = (v) => Math.max(0, Math.min(100, (v / bm.scaleMax) * 100));
    const bands = bm.bands
        .map(([a, b, c]) => `<span class="bm-band" style="left:${pct(a)}%;width:${(pct(b) - pct(a)).toFixed(1)}%;background:${c}"></span>`)
        .join('');
    return `<div class="bm-track" title="${bm.value} ${bm.unit} on a 0–${bm.scaleMax} ${bm.unit} scale">${bands}<span class="bm-marker" style="left:${pct(bm.value)}%"></span></div>`;
}
