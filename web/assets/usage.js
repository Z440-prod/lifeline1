/* Lifeline — on-device usage memory.
   Records how *this* user actually uses the app — which views they open, which
   actions they reach for — and turns it into a recency-weighted score so the
   app can reshape around habits: reorder destinations, surface the place they
   live in, and tell the on-device coach how they use the product. Every count
   is a decaying score (half-life ~7 days) so the app tracks *current* habits,
   not ancient ones. Local-only; the server never sees a tap. */

const LS = 'lifeline.usage';
const HALF_LIFE_MS = 7 * 24 * 60 * 60 * 1000; // habits fade over a week

function load() {
    try { return JSON.parse(localStorage.getItem(LS) || '{}') || {}; }
    catch { return {}; }
}
function save(m) { try { localStorage.setItem(LS, JSON.stringify(m)); } catch { /* private mode */ } }

// Decay a stored { s, t } entry to `now`.
function decayed(entry, now) {
    if (!entry) return 0;
    const dt = now - (entry.t || now);
    return (entry.s || 0) * Math.pow(0.5, dt / HALF_LIFE_MS);
}

export const usage = {
    /* Count one interaction with `id` (a view id or an action key). */
    record(id) {
        if (!id) return;
        const now = Date.now();
        const m = load();
        const s = decayed(m[id], now) + 1;
        m[id] = { s, t: now };
        save(m);
    },

    /* Recency-weighted score for one id. */
    score(id) { return decayed(load()[id], Date.now()); },

    /* Rank a set of ids by score, descending. Ids never used sort last, in
       their original order (stable), so a fresh user sees the default order. */
    rank(ids) {
        const now = Date.now();
        const m = load();
        return [...ids].sort((a, b) => decayed(m[b], now) - decayed(m[a], now));
    },

    /* The single most-used id from a set, or null if none has any history. */
    top(ids) {
        const now = Date.now();
        const m = load();
        let best = null, bestScore = 0;
        for (const id of ids) {
            const sc = decayed(m[id], now);
            if (sc > bestScore) { bestScore = sc; best = id; }
        }
        return best;
    },

    /* A short plain-language summary of the top destinations, for the coach's
       context so the AI's guidance reflects how the user actually navigates. */
    summary(labelFor = (x) => x, ids = null) {
        const now = Date.now();
        const m = load();
        const entries = Object.entries(m)
            .map(([id, e]) => [id, decayed(e, now)])
            .filter(([id, sc]) => sc > 0.5 && (!ids || ids.includes(id)))
            .sort((a, b) => b[1] - a[1])
            .slice(0, 3)
            .map(([id]) => labelFor(id));
        return entries.length ? entries.join(', ') : '';
    },
};
