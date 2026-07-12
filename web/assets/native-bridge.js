/* Lifeline native bridge glue.
 *
 * The web app talks to the phone through a handful of `window.Lifeline*` objects
 * (IAP, notifications, on-device AI, device facts, native sign-in, health read).
 * On the web those are simply absent and the app falls back gracefully. Inside
 * the iOS/Android shell they are provided by the `LifelineNative` Capacitor
 * plugin (see native/plugins/lifeline-native) plus two official plugins
 * (@capacitor/local-notifications, @capacitor/device).
 *
 * This module is the single place that wires those `window.Lifeline*` objects
 * from the available plugins. It runs on every page load and no-ops entirely
 * when there's no Capacitor runtime — so it's safe to ship in the web build and
 * it "just lights up" once the app runs inside the native shell (thin-client
 * mode loads this same deployed web app, so this is exactly where the wiring
 * belongs). Import it once, early, from the app entry point.
 */

const cap = () => (typeof window !== 'undefined' ? window.Capacitor : undefined);
const plugins = () => cap()?.Plugins || {};
const LN = () => plugins().LifelineNative;          // our custom plugin
const LocalNotifications = () => plugins().LocalNotifications; // @capacitor/local-notifications
const Device = () => plugins().Device;              // @capacitor/device

/* Only wire a bridge once, and never clobber one a newer shell already set. */
function define(name, value) {
    if (typeof window === 'undefined') return;
    if (window[name]) return;
    window[name] = value;
}

export function installNativeBridges() {
    if (!cap()?.isNativePlatform?.()) return; // web / dev browser → leave everything unset

    // ── In-app purchases ────────────────────────────────────────────────────
    // The Plans page calls purchase(tier) and redeems the result at
    // POST /billing/store-receipt. LifelineNative.purchase runs the native
    // StoreKit 2 / Play Billing sheet and returns { platform, receipt }.
    if (LN()?.purchase) {
        define('LifelineIAP', {
            purchase: async (tier) => {
                const res = await LN().purchase({ tier });
                return { platform: res.platform, receipt: res.receipt };
            },
        });
    }

    // ── Daily notifications ─────────────────────────────────────────────────
    // Prefer our plugin (it owns the daily schedule + rationale); otherwise
    // adapt the official @capacitor/local-notifications plugin to the shape
    // web/assets/notify.js expects.
    if (LN()?.scheduleDaily) {
        define('LifelineNotifications', {
            permission: () => (LN().__perm || 'default'),
            requestPermission: async () => (await LN().requestNotificationPermission()).granted,
            scheduleDaily: (hour, minute) => LN().scheduleDaily({ hour, minute }),
            cancelDaily: () => LN().cancelDaily(),
            show: async (title, body) => LN().showNotification({ title, body }),
        });
    } else if (LocalNotifications()) {
        const P = LocalNotifications();
        const DAILY_ID = 4242;
        define('LifelineNotifications', {
            permission: () => 'default',
            requestPermission: async () => (await P.requestPermissions()).display === 'granted',
            scheduleDaily: (hour, minute) => P.schedule({
                notifications: [{
                    id: DAILY_ID,
                    title: 'Your Lifeline is ready',
                    body: 'Open Lifeline for today’s note.',
                    schedule: { on: { hour, minute }, allowWhileIdle: true, repeats: true },
                }],
            }),
            cancelDaily: () => P.cancel({ notifications: [{ id: DAILY_ID }] }),
            show: async (title, body) => P.schedule({
                notifications: [{ id: Date.now() % 100000, title, body, schedule: { at: new Date(Date.now() + 500) } }],
            }),
        });
    }

    // ── On-device AI (Gemma) ────────────────────────────────────────────────
    if (LN()?.aiGenerate) {
        define('LifelineLocalAI', {
            isReady: () => !!LN().__aiReady,
            download: async (modelId, onProgress) => {
                // The plugin streams progress via a listener; bridge it to the cb.
                const handle = LN().addListener?.('aiDownloadProgress', (e) => onProgress?.(e.percent));
                try { await LN().aiDownload({ modelId }); LN().__aiReady = true; }
                finally { handle?.remove?.(); }
            },
            generate: async (prompt, opts) => {
                const res = await LN().aiGenerate({ prompt, system: opts?.system, context: opts?.context, maxTokens: opts?.maxTokens ?? 512 });
                return res?.text ?? null;
            },
            remove: async () => { await LN().aiRemove(); LN().__aiReady = false; },
        });
    }

    // ── Device profile (feeds the capability scanner) ───────────────────────
    // Our plugin gives exact RAM/chipset/NPU; fall back to @capacitor/device.
    if (LN()?.deviceProfile) {
        // Populated asynchronously; the scanner reads window.LifelineDevice.profile.
        define('LifelineDevice', { profile: null });
        LN().deviceProfile().then((p) => { window.LifelineDevice.profile = p; }).catch(() => {});
    } else if (Device()) {
        define('LifelineDevice', { profile: null });
        Device().getInfo().then((info) => {
            window.LifelineDevice.profile = {
                ram_gb: info.memUsed ? undefined : undefined, // official plugin doesn't expose total RAM
                cores: (typeof navigator !== 'undefined' && navigator.hardwareConcurrency) || undefined,
                chipset: info.model,
                os: info.platform,
                os_version: info.osVersion,
                has_npu: undefined,
                ai_backends: [],
            };
        }).catch(() => {});
    }

    // ── Native sign-in (Apple / Google) ─────────────────────────────────────
    // Returns a real OIDC id-token the backend verifies at POST /account/oauth.
    if (LN()?.signInApple || LN()?.signInGoogle) {
        define('LifelineSignIn', {
            apple: LN().signInApple ? async () => (await LN().signInApple()).idToken : undefined,
            google: LN().signInGoogle ? async () => (await LN().signInGoogle()).idToken : undefined,
        });
    }

    // ── Health read (HealthKit / Health Connect) ────────────────────────────
    // Returns the same signal shape web/assets/engine.js simulates, so the
    // on-device insights engine runs on real sensor data with no other change.
    if (LN()?.readHealth) {
        define('LifelineHealth', {
            authorize: () => LN().requestHealthPermission(),
            read: async () => {
                try { return await LN().readHealth(); } catch { return null; }
            },
        });
    }

    // ── App Attest (optional hardening) ─────────────────────────────────────
    if (LN()?.attest) {
        define('LifelineAttest', { attest: (challenge) => LN().attest({ challenge }) });
    }
}
