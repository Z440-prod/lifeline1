/* Lifeline sound engine.
   Tiny synthesized UI sounds — no audio files, nothing to download, nothing
   that can autoplay. The AudioContext is only created after a user gesture
   (store-compliant), volumes stay low and short so it reads "expensive
   hardware click", not "casino". One master toggle, persisted. */

const LS = 'lifeline.sounds';
let ctx = null;
let on = (localStorage.getItem(LS) ?? '1') === '1';

function ac() {
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return null;
    if (!ctx) ctx = new AC();
    if (ctx.state === 'suspended') ctx.resume();
    return ctx;
}

function env(t0, dur, peak) {
    const g = ctx.createGain();
    g.gain.setValueAtTime(0.0001, t0);
    g.gain.exponentialRampToValueAtTime(peak, t0 + 0.008);
    g.gain.exponentialRampToValueAtTime(0.0001, t0 + dur);
    g.connect(ctx.destination);
    return g;
}

function tone(freq, dur, { type = 'sine', peak = 0.08, delay = 0, glideTo = null } = {}) {
    if (!on || !ac()) return;
    const t0 = ctx.currentTime + delay;
    const o = ctx.createOscillator();
    o.type = type;
    o.frequency.setValueAtTime(freq, t0);
    if (glideTo) o.frequency.exponentialRampToValueAtTime(glideTo, t0 + dur);
    o.connect(env(t0, dur, peak));
    o.start(t0);
    o.stop(t0 + dur + 0.03);
}

export const sound = {
    get enabled() { return on; },
    setEnabled(v) {
        on = !!v;
        localStorage.setItem(LS, on ? '1' : '0');
        if (on) this.tap();
    },

    /* fingertip on glass — tabs, rows */
    tick() { tone(1900, 0.035, { peak: 0.045 }); },
    /* button press — slightly weightier */
    tap() { tone(940, 0.05, { type: 'triangle', peak: 0.07 }); },
    /* success — a small major arpeggio */
    chime() {
        tone(659, 0.14, { peak: 0.09 });
        tone(880, 0.18, { peak: 0.08, delay: 0.07 });
        tone(1319, 0.26, { peak: 0.07, delay: 0.14 });
    },
    /* score logged — rising sweep under the confetti */
    whoosh() { tone(240, 0.22, { type: 'sawtooth', peak: 0.035, glideTo: 960 }); },
    /* confetti pops */
    pop(n = 5) {
        for (let i = 0; i < n; i++) {
            tone(420 + Math.random() * 520, 0.045, { type: 'square', peak: 0.035, delay: i * 0.035 });
        }
    },
    /* donation — the friendly coin */
    coin() {
        tone(988, 0.07, { type: 'square', peak: 0.06 });
        tone(1319, 0.16, { type: 'square', peak: 0.06, delay: 0.08 });
    },
};

/* Global press feedback: every .btn and .tab gets a sound without each call
   site remembering to ask. Gesture-driven by construction. */
export function armGlobalSounds() {
    document.addEventListener('pointerdown', (e) => {
        const t = e.target.closest?.('.btn, .tab, .seg button, .suggest button, .sheet-row');
        if (!t) return;
        if (t.classList.contains('tab') || t.classList.contains('sheet-row')) sound.tick();
        else sound.tap();
    }, { passive: true });
}
