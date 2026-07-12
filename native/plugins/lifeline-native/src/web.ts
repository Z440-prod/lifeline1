import { WebPlugin } from '@capacitor/core';

import type { LifelineNativePlugin } from './definitions';

/**
 * Web fallback. None of these capabilities exist in a plain browser, so every
 * method rejects with `unimplemented`. The web app already feature-detects the
 * `window.Lifeline*` bridges (built from this plugin only on native), so it
 * never calls these — this class exists so `registerPlugin` resolves cleanly
 * during web development.
 */
export class LifelineNativeWeb extends WebPlugin implements LifelineNativePlugin {
    private nope(name: string): Promise<never> {
        return Promise.reject(this.unimplemented(`${name} is only available in the native Lifeline app.`));
    }

    purchase(): Promise<{ platform: 'apple' | 'google'; receipt: string }> { return this.nope('purchase'); }
    requestNotificationPermission(): Promise<{ granted: boolean }> { return this.nope('requestNotificationPermission'); }
    scheduleDaily(): Promise<void> { return this.nope('scheduleDaily'); }
    cancelDaily(): Promise<void> { return this.nope('cancelDaily'); }
    showNotification(): Promise<void> { return this.nope('showNotification'); }
    aiDownload(): Promise<void> { return this.nope('aiDownload'); }
    aiGenerate(): Promise<{ text: string }> { return this.nope('aiGenerate'); }
    aiRemove(): Promise<void> { return this.nope('aiRemove'); }
    deviceProfile(): Promise<never> { return this.nope('deviceProfile'); }
    signInApple(): Promise<{ idToken: string }> { return this.nope('signInApple'); }
    signInGoogle(): Promise<{ idToken: string }> { return this.nope('signInGoogle'); }
    requestHealthPermission(): Promise<{ granted: boolean }> { return this.nope('requestHealthPermission'); }
    readHealth(): Promise<never> { return this.nope('readHealth'); }
    attest(): Promise<{ keyId: string; attestation: string }> { return this.nope('attest'); }
}
