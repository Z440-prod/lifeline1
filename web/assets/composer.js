/* Lifeline — the layout composer (server-driven / generative UI, done the
   compliant way).

   THE ARCHITECTURE: the app ships a fixed catalog of render-able blocks and a
   fixed renderer. This composer emits a *declarative manifest* — plain data
   describing which blocks to show, in what order, which to hide, and which
   contextual block to surface — computed from what the app knows about the
   user (their Conductor mode, focus, rank, uploaded data, and habits). The
   renderer just draws the manifest.

   Nothing here downloads or executes code. A manifest is data, exactly like a
   feature flag, an A/B config, or a server-driven-UI payload — the pattern
   Airbnb, Lyft, Spotify, and every remote-config system already ship on the
   App Store (Apple Guideline 2.5.2 permits config/data driving a fixed
   interpreter; it forbids downloading executable code that changes the app's
   purpose — which this never does).

   Today this composer is rules. Because its output is a plain JSON manifest,
   the on-device LLM can later emit the SAME shape and the renderer won't change
   — that's the path to "the AI arranges the app," with zero new store risk. */

/* Compose the Today screen's insight-block manifest for this user.
   ctx: { lead, focus, dataRichness, labs, sources, league, prestige,
          usesArena (0..1), available: Set<blockId> }
   Returns: { blocks: [id...], hidden: [{id,reason}], surface: {id,...}|null,
              why: string } */
export function composeToday(ctx) {
    const { lead, dataRichness, labs = 0, sources = 0, league, usesArena = 0 } = ctx;
    const available = ctx.available || new Set(['readiness', 'age', 'circadian']);
    const has = (id) => available.has(id);

    const hidden = [];
    const reasons = [];

    // 1. The focus-led block always leads.
    const ordered = [];
    if (lead && has(lead)) { ordered.push(lead); reasons.push(`${lead} leads because it's your focus`); }

    // 2. Relevance order for the rest.
    const rest = ['readiness', 'age', 'circadian'].filter((id) => id !== lead && has(id));

    // 3. Declutter for sparse users: the Lifeline Age block is least actionable
    //    without labs, so hide it for a sparse/new user unless it's their focus.
    const hideAge = dataRichness === 'sparse' && labs === 0 && lead !== 'age';
    for (const id of rest) {
        if (id === 'age' && hideAge) { hidden.push({ id: 'age', reason: 'no labs yet — hidden until you add data' }); continue; }
        ordered.push(id);
    }
    if (hideAge) reasons.push('Lifeline Age hidden until you connect labs');

    // 4. Surface ONE contextual block based on what's missing or hot.
    let surface = null;
    if (sources === 0) {
        surface = { id: 'connect-source', title: 'Connect a health source', body: 'Link Apple Health, Google, or Whoop to sharpen every number.', cta: 'sources', accentReason: 'you have no source connected yet' };
        reasons.push('surfacing a connect-source prompt (no sources yet)');
    } else if (labs === 0 && dataRichness !== 'sparse') {
        surface = { id: 'add-labs', title: 'Add your labs', body: 'Upload bloodwork to unlock a precise Lifeline Age, plotted on-device.', cta: 'vault', accentReason: 'you have sources but no labs' };
        reasons.push('surfacing an add-labs prompt (sources but no labs)');
    } else if (usesArena > 0.6 && league) {
        surface = { id: 'arena-push', title: `Defend your ${league.name} rank`, body: 'You live in the Arena — log today to hold your league.', cta: 'arena', accentReason: 'you use the Arena heavily' };
        reasons.push('surfacing an Arena nudge (heavy Arena user)');
    }

    return {
        blocks: ordered,
        hidden,
        surface,
        why: reasons.join('; ') || 'default arrangement',
    };
}
