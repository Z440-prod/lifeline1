/**
 * LifelineNative — the single Capacitor plugin that backs every `window.Lifeline*`
 * bridge the web app calls. The web glue (web/assets/native-bridge.js) adapts
 * these methods into those bridges; keep the two in sync.
 */
export interface LifelineNativePlugin {
    // ── In-app purchases (StoreKit 2 / Play Billing) ────────────────────────
    /** Run the native purchase sheet for a tier. Rejects on cancel. */
    purchase(options: { tier: 'pro' | 'elite' }): Promise<{ platform: 'apple' | 'google'; receipt: string }>;

    // ── Notifications (daily check-in) ──────────────────────────────────────
    requestNotificationPermission(): Promise<{ granted: boolean }>;
    scheduleDaily(options: { hour: number; minute: number }): Promise<void>;
    cancelDaily(): Promise<void>;
    showNotification(options: { title: string; body: string }): Promise<void>;

    // ── On-device AI (Gemma via MediaPipe LLM / Core ML) ────────────────────
    aiDownload(options: { modelId: string }): Promise<void>;
    aiGenerate(options: { prompt: string; system?: string; context?: string; maxTokens?: number }): Promise<{ text: string }>;
    aiRemove(): Promise<void>;
    /** Emits `aiDownloadProgress` events: { percent: number }. */
    addListener(eventName: 'aiDownloadProgress', cb: (e: { percent: number }) => void): Promise<{ remove: () => void }>;

    // ── Device profile (feeds the capability scanner) ───────────────────────
    deviceProfile(): Promise<{
        ram_gb: number;
        cores: number;
        chipset: string;
        os: 'ios' | 'android';
        os_version: string;
        has_npu: boolean;
        ai_backends: string[];
    }>;

    // ── Native sign-in (returns a real OIDC id-token the backend verifies) ──
    signInApple(): Promise<{ idToken: string }>;
    signInGoogle(): Promise<{ idToken: string }>;

    // ── Health (HealthKit / Health Connect) ─────────────────────────────────
    requestHealthPermission(): Promise<{ granted: boolean }>;
    /** Today's signals in the shape web/assets/engine.js expects. */
    readHealth(): Promise<{
        chrono_age?: number;
        resting_heart_rate?: number;
        hrv_ms?: number;
        sleep_hours?: number;
        daily_steps?: number;
        sleep_performance?: number;
        prior_strain?: number;
        sleep_midpoint?: number;
    }>;

    // ── App Attest (optional device-integrity hardening) ────────────────────
    attest(options: { challenge: string }): Promise<{ keyId: string; attestation: string }>;
}
