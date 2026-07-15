/* Lifeline — the "feel" slider.
   A tactile daily check-in: drag to log how charged you feel. Rule of three
   zones (Drained / Steady / Charged), a rising-pitch detent tick + a light
   haptic each time the thumb crosses a notch, a spring thumb with a live glow,
   and a satisfying release burst. Synthesized sound only (offline/CSP-safe),
   keyboard-accessible, and it respects prefers-reduced-motion for the ripple.
   Designed to feel like premium hardware, not a toy. */
import { sound } from './sound.js';

const ZONES = [
    { max: 33, label: 'Drained', color: 'var(--sleep)' },
    { max: 66, label: 'Steady', color: 'var(--tint)' },
    { max: 101, label: 'Charged', color: 'var(--activity)' },
];
const zoneOf = (v) => ZONES.find((z) => v < z.max) || ZONES[2];
const clamp = (n) => Math.max(0, Math.min(100, n));

export function mountFeelSlider(host, { value = 50, onCommit } = {}) {
    if (!host) return;
    let v = clamp(value);
    let lastNotch = -1;
    host.classList.add('feel');
    host.innerHTML = `
        <div class="feel-head">
            <span class="feel-eyebrow">Daily check-in</span>
            <span class="feel-read"><b class="feel-val tnum">${Math.round(v)}</b><span class="feel-zone">${zoneOf(v).label}</span></span>
        </div>
        <div class="feel-track" role="slider" tabindex="0"
             aria-label="How charged do you feel today"
             aria-valuemin="0" aria-valuemax="100" aria-valuenow="${Math.round(v)}">
            <div class="feel-fill"></div>
            <div class="feel-thumb"></div>
            <div class="feel-burst"></div>
        </div>
        <div class="feel-zones"><span>Drained</span><span>Steady</span><span>Charged</span></div>`;

    const track = host.querySelector('.feel-track');
    const fill = host.querySelector('.feel-fill');
    const thumb = host.querySelector('.feel-thumb');
    const burst = host.querySelector('.feel-burst');
    const valEl = host.querySelector('.feel-val');
    const zoneEl = host.querySelector('.feel-zone');

    const paint = () => {
        const z = zoneOf(v);
        fill.style.width = v + '%';
        fill.style.background = z.color;
        thumb.style.left = v + '%';
        thumb.style.setProperty('--glow', z.color);
        valEl.textContent = Math.round(v);
        zoneEl.textContent = z.label;
        zoneEl.style.color = z.color;
        track.setAttribute('aria-valuenow', Math.round(v));
        track.setAttribute('aria-valuetext', z.label);
    };
    paint();

    const feedbackAt = () => {
        const notch = Math.round(v / 5); // a detent every 5%
        if (notch === lastNotch) return;
        lastNotch = notch;
        sound.detent(v / 100);
        if (navigator.vibrate) { try { navigator.vibrate(8); } catch (e) { /* unsupported */ } }
        thumb.classList.remove('pulse'); void thumb.offsetWidth; thumb.classList.add('pulse');
    };

    const setFromX = (clientX) => {
        const r = track.getBoundingClientRect();
        v = clamp(((clientX - r.left) / r.width) * 100);
        feedbackAt();
        paint();
    };

    const commit = () => {
        sound.chime();
        burst.style.left = v + '%';
        burst.classList.remove('go'); void burst.offsetWidth; burst.classList.add('go');
        if (onCommit) onCommit(Math.round(v), zoneOf(v).label);
    };

    let dragging = false;
    // Pointer capture keeps move/up on the track element itself, so the
    // listeners are scoped to this node and don't leak on re-render.
    track.addEventListener('pointerdown', (e) => {
        dragging = true; track.classList.add('active');
        track.setPointerCapture?.(e.pointerId);
        setFromX(e.clientX);
    });
    track.addEventListener('pointermove', (e) => { if (dragging) setFromX(e.clientX); });
    const end = () => { if (!dragging) return; dragging = false; track.classList.remove('active'); commit(); };
    track.addEventListener('pointerup', end);
    track.addEventListener('pointercancel', end);

    track.addEventListener('keydown', (e) => {
        if (e.key === 'ArrowRight' || e.key === 'ArrowUp') { v = clamp(v + 5); feedbackAt(); paint(); e.preventDefault(); }
        else if (e.key === 'ArrowLeft' || e.key === 'ArrowDown') { v = clamp(v - 5); feedbackAt(); paint(); e.preventDefault(); }
        else if (e.key === 'Enter' || e.key === ' ') { commit(); e.preventDefault(); }
    });
}
