/* Lifeline notifications — the once-a-day check-in.
 *
 * Opt-in only (both stores require it, and it's the right thing): nothing fires
 * until the user turns it on in Settings and grants OS permission. Once a local
 * day, when the app is opened, it shows the AI-written daily anecdote — and on
 * the native shells it also schedules a real background reminder so the note
 * arrives even if the app isn't open.
 *
 * Two backends, same as the rest of the app:
 *   • Native bridge — `window.LifelineNotifications`, injected by the iOS/Android
 *     shell (Capacitor Local Notifications). Handles OS permission and true
 *     daily background scheduling.
 *   • Web fallback — the Notifications API via the service worker registration,
 *     fired on app-open (a browser can't reliably wake itself in the background
 *     without Web Push infrastructure).
 *
 * The daily gate (one notification per local day) lives here so both backends
 * honor it. No health data is ever sent anywhere — the anecdote text is
 * generated on-device and only handed to the OS notification center. */

const LS_ENABLED = 'lifeline.notify';
const LS_LAST = 'lifeline.notify.last';   // YYYY-MM-DD of the last daily fire
const LS_HOUR = 'lifeline.notify.hour';   // preferred local hour (native scheduling)

const bridge = () => (typeof window !== 'undefined' ? window.LifelineNotifications : undefined);
const today = () => new Date().toISOString().slice(0, 10);

const ICON = '/assets/icon-notification.png'; // optional; browsers fall back gracefully

export const notify = {
    /* Can this environment show notifications at all? */
    supported() {
        return !!bridge() || (typeof Notification !== 'undefined');
    },

    enabled() {
        return localStorage.getItem(LS_ENABLED) === '1';
    },

    /* The OS-level permission state, best-effort across backends. */
    permission() {
        if (bridge()?.permission) {
            try { return bridge().permission(); } catch { /* fall through */ }
        }
        return typeof Notification !== 'undefined' ? Notification.permission : 'denied';
    },

    hour() {
        const h = parseInt(localStorage.getItem(LS_HOUR) || '9', 10);
        return Number.isNaN(h) ? 9 : Math.min(23, Math.max(0, h));
    },
    setHour(h) { localStorage.setItem(LS_HOUR, String(h)); this.scheduleDaily(); },

    /* Turn the daily check-in on: request OS permission, then persist the
       preference and (on native) schedule the background reminder. Returns
       whether it's now enabled. */
    async enable() {
        let granted = false;
        if (bridge()?.requestPermission) {
            try { granted = !!(await bridge().requestPermission()); } catch { granted = false; }
        } else if (typeof Notification !== 'undefined') {
            try { granted = (await Notification.requestPermission()) === 'granted'; } catch { granted = false; }
        }
        if (!granted) return false;
        localStorage.setItem(LS_ENABLED, '1');
        this.scheduleDaily();
        return true;
    },

    disable() {
        localStorage.setItem(LS_ENABLED, '0');
        try { bridge()?.cancelDaily?.(); } catch { /* ignore */ }
    },

    /* Ask the native shell to schedule a repeating daily local notification at
       the user's preferred hour. No-op on the web (fired on app-open instead). */
    scheduleDaily() {
        if (!this.enabled()) return;
        try { bridge()?.scheduleDaily?.(this.hour(), 0); } catch { /* ignore */ }
    },

    /* Has today's note already been shown? Enforces once-per-day. */
    sentToday() {
        return localStorage.getItem(LS_LAST) === today();
    },

    /* Show a notification now and stamp today's date. Prefers the native bridge,
       then the service-worker registration, then a plain Notification. */
    async show(title, body) {
        localStorage.setItem(LS_LAST, today());
        if (bridge()?.show) {
            try { await bridge().show(title, body); return true; } catch { /* fall through */ }
        }
        if (this.permission() !== 'granted') return false;
        try {
            const reg = await navigator.serviceWorker?.getRegistration?.();
            if (reg?.showNotification) {
                await reg.showNotification(title, { body, icon: ICON, tag: 'lifeline-daily', badge: ICON });
                return true;
            }
        } catch { /* fall through */ }
        try {
            // eslint-disable-next-line no-new
            new Notification(title, { body, icon: ICON, tag: 'lifeline-daily' });
            return true;
        } catch { return false; }
    },
};
