/* Lifeline on-device AI.
   Runs the coach entirely on the user's phone when the device can handle it —
   so answers are instant, fully private, and work with no internet at all.

   Two backends, chosen by capability:
     1. Native bridge  — `window.LifelineLocalAI`, injected by the iOS/Android
        shell. This is the real path on premium phones: it downloads a Gemma
        model through the OS ML runtime (MediaPipe LLM Inference / Core ML) and
        runs true on-device inference.
     2. Simulated (web) — the browser build can't ship multi-GB weights under
        the app's strict CSP, so here we simulate the download and answer from a
        small on-device rule engine. Same UX and code path; swapped for the real
        model the moment the app runs inside the native shell. (Mirrors how the
        store IAP and OAuth flows are simulated in the web build.)

   State (which model, ready/among) is persisted per-device in localStorage.
   The coach calls `generate()`; if a local model is ready it answers locally,
   otherwise it returns null and the caller falls back to the cloud proxy. */

import { scanDevice, eligibleModels } from './device.js';
import { api } from './api.js';

const LS = 'lifeline.localai';
const bridge = () => (typeof window !== 'undefined' ? window.LifelineLocalAI : undefined);
const hasNativeBridge = () => typeof bridge() !== 'undefined';

function load() {
    try { return JSON.parse(localStorage.getItem(LS) || 'null') || {}; }
    catch { return {}; }
}
function save(s) { localStorage.setItem(LS, JSON.stringify(s)); }

let scan = null;      // cached device scan
let catalog = null;   // cached /ai/local-models

export const localAI = {
    /* Discover what this device can run. Safe to call repeatedly. */
    async probe() {
        if (!scan) scan = await scanDevice();
        if (!catalog) {
            const res = await api.localModels();
            catalog = res.status === 200 ? res.data : null;
        }
        const models = eligibleModels(scan, catalog);
        const st = load();
        return {
            scan,
            catalog,
            models,                       // eligible, best-first
            eligible: models.length > 0,
            backend: hasNativeBridge() ? 'native' : (scan?.backends.includes('webgpu') ? 'webgpu' : 'simulated'),
            state: st.state || 'idle',    // idle | downloading | ready | error
            modelId: st.modelId || null,
            progress: st.progress || 0,
        };
    },

    /* Is a local model installed and ready to answer right now? */
    ready() {
        const st = load();
        if (st.state !== 'ready') return false;
        // If a native bridge is present it is the source of truth.
        if (hasNativeBridge()) { try { return !!bridge().isReady?.(); } catch { return false; } }
        return true; // simulated model persists as ready
    },

    activeModelId() { return load().modelId || null; },

    /* Download + install a model. `onProgress(pct, label)` is called as it goes.
       Resolves to { ok, modelId } or { ok:false, error }. */
    async install(modelId, onProgress = () => {}) {
        const info = await this.probe();
        const model = info.models.find((m) => m.id === modelId) || info.models[0];
        if (!model) return { ok: false, error: 'This device can’t run an on-device model.' };

        save({ state: 'downloading', modelId: model.id, progress: 0 });

        try {
            if (hasNativeBridge()) {
                // Real path: the native runtime streams the weights and reports
                // progress; it verifies the sha256 from the catalog itself.
                await bridge().download(model.id, (pct) => {
                    save({ state: 'downloading', modelId: model.id, progress: pct });
                    onProgress(pct, `Downloading ${model.label}…`);
                });
            } else {
                // Simulated path: page through progress so the UX is real, then
                // mark ready. No bytes actually move; the on-device rule engine
                // answers in place of Gemma until the native shell swaps it in.
                for (let p = 0; p <= 100; p += 8) {
                    await new Promise((r) => setTimeout(r, 90));
                    const pct = Math.min(100, p);
                    save({ state: 'downloading', modelId: model.id, progress: pct });
                    onProgress(pct, `Preparing ${model.label} on-device…`);
                }
            }
            save({ state: 'ready', modelId: model.id, progress: 100 });
            return { ok: true, modelId: model.id };
        } catch (e) {
            save({ state: 'error', modelId: model.id, progress: 0 });
            return { ok: false, error: e?.message || 'Download failed.' };
        }
    },

    /* Remove the installed model and free the space. */
    async remove() {
        try { if (hasNativeBridge()) await bridge().remove?.(); } catch { /* ignore */ }
        save({ state: 'idle', modelId: null, progress: 0 });
    },

    /* Answer a prompt on-device. Returns the reply string, or null if no local
       model is ready (so the caller uses the cloud proxy instead). `context`
       carries the on-device system prompt + biometric summary; it never leaves
       the device on this path. */
    async generate(prompt, context = {}) {
        if (!this.ready()) return null;
        if (hasNativeBridge()) {
            try {
                const out = await bridge().generate(prompt, {
                    system: context.system,
                    context: context.summary,
                    maxTokens: 512,
                });
                return typeof out === 'string' ? out : (out?.text ?? null);
            } catch { return null; }
        }
        // Simulated on-device inference: a compact rule engine over the same
        // biometric summary the real model would receive. Runs synchronously in
        // the page — no network — so it faithfully demonstrates the offline path.
        return simulatedInfer(prompt, context);
    },

    /* THE AI CODES THE APP — safely. Ask the on-device model to author the
       layout as a JSON manifest from the user's profile. Returns a parsed
       object (UNTRUSTED — the caller MUST pass it through validateManifest) or
       null if no local model is ready. The prompt hard-constrains the model to a
       closed vocabulary, and the caller's validator enforces it regardless of
       what the model actually emits — so a bad/hallucinated answer can never
       break or compromise the app. */
    async composeLayout(profile = {}) {
        if (!this.ready()) return null;
        const system = 'You are Lifeline\'s on-device layout composer. Output ONLY a JSON object, no prose. '
            + 'Shape: {"blocks":[ids],"surface":"id"|null,"accent":"#rrggbb"|null}. '
            + 'blocks ids allowed: "readiness","age","circadian". surface ids allowed: '
            + '"connect-source","add-labs","arena-push", or null. Order blocks by what matters most for THIS user. '
            + 'Never invent ids. Never output anything but the JSON object.';
        const prompt = `User profile: ${JSON.stringify(profile)}. Compose their Today layout.`;
        let out;
        try { out = await this.generate(prompt, { system, summary: JSON.stringify(profile) }); }
        catch { return null; }
        if (typeof out !== 'string') return null;
        // Extract the first {...} so stray tokens around the JSON don't break parsing.
        const m = out.match(/\{[\s\S]*\}/);
        if (!m) return null;
        try { return JSON.parse(m[0]); } catch { return null; }
    },
};

/* A small, honest on-device responder. Not Gemma — a deterministic rule engine
   that reads the biometric summary and the question and gives a clinical-first
   answer, so the offline coach is demonstrably functional in the web build. The
   native bridge replaces this with real Gemma weights on supported phones. */
function simulatedInfer(prompt, context) {
    const s = context.signals || {};
    const q = (prompt || '').toLowerCase();
    const bits = [];
    const has = (...ks) => ks.some((k) => q.includes(k));

    if (has('sleep', 'rest', 'tired')) {
        bits.push(`You logged ${s.sleep_hours ?? '—'} h of sleep with a ${s.sleep_performance ?? '—'}% performance score.`);
        bits.push(s.sleep_hours && s.sleep_hours < 7
            ? 'That’s below your 7–9 h target — pull your wind-down 30 minutes earlier tonight and cut screens before bed.'
            : 'That’s in a healthy band. Protect it by keeping a consistent bedtime.');
    } else if (has('heart', 'hrv', 'cardio', 'rhr', 'resting')) {
        bits.push(`Resting heart rate is ${s.resting_heart_rate ?? '—'} bpm and HRV is ${s.hrv_ms ?? '—'} ms.`);
        bits.push('Both point to your autonomic recovery. Zone-2 cardio and steady sleep nudge HRV up over weeks.');
    } else if (has('step', 'move', 'active', 'exercise', 'train', 'workout')) {
        bits.push(`You’re at ${s.daily_steps ?? '—'} steps today.`);
        bits.push(s.daily_steps && s.daily_steps < 8000
            ? 'A brisk 20-minute walk would close the gap to 8k and lift tomorrow’s readiness.'
            : 'Nice — you’re past the 8k mark that tracks with better longevity markers.');
    } else if (has('lifeline age', 'age', 'younger', 'older')) {
        bits.push('Your Lifeline Age is an additive model: resting HR, HRV, sleep, and steps each shift it in years.');
        bits.push('The fastest lever for most people is sleep consistency, then daily steps.');
    } else if (has('plan', 'today', 'tomorrow', 'should i')) {
        bits.push('Given today’s signals, keep it simple: hydrate, a Zone-2 walk, and an earlier wind-down.');
        bits.push('Small, repeatable wins move your score more than heroic one-offs.');
    } else {
        bits.push('Here’s what your on-device signals suggest:');
        bits.push(`readiness is fused from HRV (${s.hrv_ms ?? '—'} ms), resting HR (${s.resting_heart_rate ?? '—'} bpm), and sleep (${s.sleep_hours ?? '—'} h).`);
        bits.push('Ask me about your sleep, heart rate, activity, or a plan for today.');
    }
    bits.push('\n— answered privately on your device, offline.');
    return bits.join(' ');
}
