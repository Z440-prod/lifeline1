/* Lifeline service worker — the offline half of "no internet required".
 *
 * Precaches the app shell + assets and caches the server's rulebook configs, so
 * once a premium device has installed an on-device model the whole app runs
 * with no connection: the shell loads from cache, the on-device engine computes
 * the portrait from the cached rules, and the local model answers the coach.
 *
 * SECURITY: this worker is deliberately narrow. It caches ONLY the static app
 * shell/assets and an allowlist of public, user-independent rulebook GETs. It
 * never caches authenticated or user-scoped traffic — sessions, sync documents,
 * account/billing/AI-proxy calls all go network-only and are never written to
 * the cache — so no bearer token or personal data can ever be persisted here.
 * Same-origin only; cross-origin requests are passed straight through.
 */

const VERSION = 'v1';
const SHELL_CACHE = `lifeline-shell-${VERSION}`;
const RULES_CACHE = `lifeline-rules-${VERSION}`;

// The minimum needed to boot the UI offline.
const SHELL_ASSETS = [
    '/',
    '/index.html',
    '/manifest.webmanifest',
    '/assets/app.css',
    '/assets/app.js',
    '/assets/api.js',
    '/assets/engine.js',
    '/assets/charts.js',
    '/assets/sound.js',
    '/assets/device.js',
    '/assets/localai.js',
];

// Public, user-independent rulebooks — safe to cache and serve offline.
const RULEBOOK_PATHS = new Set([
    '/api/v1/insights/config',
    '/api/v1/game/config',
    '/api/v1/billing/config',
    '/api/v1/ai/policy-matrix',
    '/api/v1/ai/local-models',
]);

self.addEventListener('install', (event) => {
    event.waitUntil(
        caches.open(SHELL_CACHE)
            // Best-effort: one missing asset must not abort the whole install.
            .then((cache) => Promise.allSettled(SHELL_ASSETS.map((u) => cache.add(u))))
            .then(() => self.skipWaiting()),
    );
});

self.addEventListener('activate', (event) => {
    event.waitUntil(
        caches.keys()
            .then((keys) => Promise.all(
                keys.filter((k) => k !== SHELL_CACHE && k !== RULES_CACHE)
                    .map((k) => caches.delete(k)),
            ))
            .then(() => self.clients.claim()),
    );
});

/* Stale-while-revalidate: serve the cached copy immediately, refresh in the
   background. Only ever stores successful, basic (same-origin) responses. */
async function staleWhileRevalidate(request, cacheName) {
    const cache = await caches.open(cacheName);
    const cached = await cache.match(request);
    const network = fetch(request)
        .then((res) => {
            if (res && res.status === 200 && res.type === 'basic') {
                cache.put(request, res.clone());
            }
            return res;
        })
        .catch(() => null);
    return cached || network || fetch(request);
}

self.addEventListener('fetch', (event) => {
    const { request } = event;
    // Only GET, only same-origin. Everything else (POST/auth/sync/cross-origin)
    // goes straight to the network and is never cached.
    if (request.method !== 'GET') return;
    const url = new URL(request.url);
    if (url.origin !== self.location.origin) return;

    // Navigations: network-first so fresh deploys win, cached shell on failure
    // (this is what lets the app open with no connection).
    if (request.mode === 'navigate') {
        event.respondWith(
            fetch(request).catch(() => caches.match('/index.html').then((r) => r || caches.match('/'))),
        );
        return;
    }

    // Public rulebooks: cache so the on-device engine has its rules offline.
    if (RULEBOOK_PATHS.has(url.pathname)) {
        event.respondWith(staleWhileRevalidate(request, RULES_CACHE));
        return;
    }

    // Static assets: stale-while-revalidate for instant, offline-capable loads.
    if (url.pathname.startsWith('/assets/') || url.pathname === '/manifest.webmanifest') {
        event.respondWith(staleWhileRevalidate(request, SHELL_CACHE));
        return;
    }

    // Everything else (API auth/sync/account/billing/ai-proxy): network-only,
    // never cached — no tokens or user data ever touch the cache.
});
