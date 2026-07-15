/* Lifeline — the layout composer + safety gate (server-driven / generative UI).

   TWO jobs:
   1. composeToday(ctx)   — RULES author a layout manifest from what the app
                            knows about the user.
   2. validateManifest(raw, fallback) — the SAFETY GATE. Takes ANY manifest
                            (from the on-device AI, from rules, or from garbage)
                            and returns a guaranteed-valid, guaranteed-safe
                            manifest. This is what makes the AI-authored app
                            work 100% of the time: the AI may only CHOOSE among
                            known, pre-built, already-reviewed blocks; it can
                            never inject new UI, new text, new code, or new
                            styles. Anything it emits that isn't on the allowlist
                            is dropped; anything malformed falls back to a safe
                            default. The gate is a total function — it never
                            throws and always returns something renderable.

   Why this is store-compliant AND safe: the AI outputs DATA from a CLOSED
   VOCABULARY, never code. The renderer draws only allowlisted blocks shipped in
   the reviewed binary. See DYNAMIC_UI.md. */

// ── The closed vocabulary. The AI can pick from these and nothing else. ──────
export const BLOCK_ALLOW = ['readiness', 'age', 'circadian'];
export const TAB_ALLOW = ['portrait', 'arena', 'coach', 'vault', 'sources', 'plans', 'settings'];
// Surfaces carry app-authored copy keyed by id. The AI picks an id; it NEVER
// supplies the text, so it can't inject markup or misleading copy.
export const SURFACES = {
    'connect-source': { id: 'connect-source', title: 'Connect a health source', body: 'Link Apple Health, Google, or Whoop to sharpen every number.', cta: 'sources' },
    'add-labs': { id: 'add-labs', title: 'Add your labs', body: 'Upload bloodwork to unlock a precise Lifeline Age, plotted on-device.', cta: 'vault' },
    'arena-push': { id: 'arena-push', title: 'Defend your rank', body: 'You live in the Arena — log today to hold your league.', cta: 'arena' },
};
const HEX = /^#[0-9a-fA-F]{6}$/;

/* THE SAFETY GATE. raw = any manifest-shaped value (untrusted). fallback = a
   known-good manifest to borrow from when raw is missing/invalid. Returns a
   manifest that is ALWAYS safe to render. Never throws. */
export function validateManifest(raw, fallback) {
    const safe = {
        blocks: (fallback && Array.isArray(fallback.blocks) && fallback.blocks.length) ? fallback.blocks : ['readiness'],
        surface: (fallback && fallback.surface) || null,
        tabs: (fallback && fallback.tabs) || null,
        accent: (fallback && fallback.accent) || null,
        source: 'fallback',
    };
    if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return safe;
    try {
        // blocks: allowlisted ids only, deduped, non-empty.
        let blocks = Array.isArray(raw.blocks) ? raw.blocks.filter((b) => BLOCK_ALLOW.includes(b)) : [];
        blocks = [...new Set(blocks)];
        if (!blocks.length) blocks = safe.blocks;

        // surface: only a known id; app-owned copy. Unknown → none.
        let surface = null;
        const sid = raw.surface && typeof raw.surface === 'object' ? raw.surface.id : raw.surface;
        if (typeof sid === 'string' && SURFACES[sid]) surface = { ...SURFACES[sid] };

        // tabs: allowlisted ids only; Today always present and first.
        let tabs = Array.isArray(raw.tabs) ? [...new Set(raw.tabs.filter((t) => TAB_ALLOW.includes(t)))] : null;
        if (tabs && tabs.length) {
            tabs = tabs.filter((t) => t !== 'portrait');
            tabs.unshift('portrait');
        } else {
            tabs = null;
        }

        // accent: a strict 6-digit hex only. Anything else → no override (no CSS injection).
        const accent = typeof raw.accent === 'string' && HEX.test(raw.accent) ? raw.accent : null;

        return { blocks, surface, tabs, accent, source: 'validated' };
    } catch {
        return safe; // total function: never throw
    }
}

/* RULES author of the Today manifest (the deterministic baseline + fallback). */
export function composeToday(ctx) {
    const { lead, dataRichness, labs = 0, sources = 0, league, usesArena = 0 } = ctx;
    const available = ctx.available || new Set(BLOCK_ALLOW);
    const has = (id) => available.has(id);

    const ordered = [];
    if (lead && has(lead)) ordered.push(lead);
    const rest = BLOCK_ALLOW.filter((id) => id !== lead && has(id));
    const hideAge = dataRichness === 'sparse' && labs === 0 && lead !== 'age';
    const hidden = [];
    for (const id of rest) {
        if (id === 'age' && hideAge) { hidden.push({ id: 'age', reason: 'no labs yet' }); continue; }
        ordered.push(id);
    }

    let surface = null;
    if (sources === 0) surface = { ...SURFACES['connect-source'] };
    else if (labs === 0 && dataRichness !== 'sparse') surface = { ...SURFACES['add-labs'] };
    else if (usesArena > 0.6 && league) surface = { ...SURFACES['arena-push'], title: `Defend your ${league.name} rank` };

    return { blocks: ordered, hidden, surface, tabs: null, accent: null, source: 'rules' };
}
