import { registerPlugin } from '@capacitor/core';

import type { LifelineNativePlugin } from './definitions';

/**
 * The `LifelineNative` plugin. `registerPlugin` returns the native
 * implementation inside the shell, or the web fallback (see web.ts) elsewhere.
 */
const LifelineNative = registerPlugin<LifelineNativePlugin>('LifelineNative', {
    web: () => import('./web').then((m) => new m.LifelineNativeWeb()),
});

export * from './definitions';
export { LifelineNative };
