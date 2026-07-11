/* Lifeline API client.
   Owns the device identity (a stable UUID in localStorage), the session
   token, and every call to the Antigravity backend. The app is served by the
   backend itself, so all paths are same-origin. */

const BASE = '/api/v1';
const LS_DEVICE = 'lifeline.device_id';

export const identity = {
    get deviceId() {
        let id = localStorage.getItem(LS_DEVICE);
        if (!id) { id = crypto.randomUUID(); localStorage.setItem(LS_DEVICE, id); }
        return id;
    },
    reset() {
        localStorage.clear();
        location.reload();
    },
};

/* ── Device crypto ──────────────────────────────────────────────────────────
   A real ECDSA P-256 keypair (sync payloads are signed for the server to
   verify against the registered public key) and a local AES-256-GCM key
   (vault blobs are true ciphertext — the server can never read them). Keys
   persist as JWKs on this device only. */
const LS_KEYS = 'lifeline.keys';
const b64 = {
    enc: (buf) => btoa(String.fromCharCode(...new Uint8Array(buf))),
    dec: (s) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0)),
};

/* WebCrypto emits ECDSA signatures as raw r||s (P1363); ring verifies ASN.1
   DER. Minimal conversion: two DER INTEGERs inside a SEQUENCE. */
function p1363ToDer(sig) {
    const half = sig.length / 2;
    const int = (bytes) => {
        let i = 0;
        while (i < bytes.length - 1 && bytes[i] === 0) i++;
        let v = bytes.slice(i);
        if (v[0] & 0x80) v = Uint8Array.from([0, ...v]);
        return Uint8Array.from([0x02, v.length, ...v]);
    };
    const r = int(sig.slice(0, half));
    const s = int(sig.slice(half));
    return Uint8Array.from([0x30, r.length + s.length, ...r, ...s]);
}

export const deviceCrypto = {
    signKey: null,
    aesKey: null,
    publicKeyB64: null,

    async init() {
        if (this.signKey) return;
        const stored = localStorage.getItem(LS_KEYS);
        const ecdsa = { name: 'ECDSA', namedCurve: 'P-256' };
        if (stored) {
            const jwks = JSON.parse(stored);
            this.signKey = await crypto.subtle.importKey('jwk', jwks.sign, ecdsa, true, ['sign']);
            const pub = await crypto.subtle.importKey('jwk', jwks.verify, ecdsa, true, ['verify']);
            this.publicKeyB64 = b64.enc(await crypto.subtle.exportKey('raw', pub));
            this.aesKey = await crypto.subtle.importKey('jwk', jwks.aes, { name: 'AES-GCM' }, true, ['encrypt', 'decrypt']);
            return;
        }
        const pair = await crypto.subtle.generateKey(ecdsa, true, ['sign', 'verify']);
        const aes = await crypto.subtle.generateKey({ name: 'AES-GCM', length: 256 }, true, ['encrypt', 'decrypt']);
        this.signKey = pair.privateKey;
        this.aesKey = aes;
        this.publicKeyB64 = b64.enc(await crypto.subtle.exportKey('raw', pair.publicKey));
        localStorage.setItem(LS_KEYS, JSON.stringify({
            sign: await crypto.subtle.exportKey('jwk', pair.privateKey),
            verify: await crypto.subtle.exportKey('jwk', pair.publicKey),
            aes: await crypto.subtle.exportKey('jwk', aes),
        }));
    },

    /* AES-GCM: WebCrypto returns ciphertext||tag(16). The API stores them
       separately, so split. */
    async encrypt(obj) {
        const iv = crypto.getRandomValues(new Uint8Array(12));
        const pt = new TextEncoder().encode(JSON.stringify(obj));
        const ct = new Uint8Array(await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, this.aesKey, pt));
        const blob = ct.slice(0, ct.length - 16);
        const tag = ct.slice(ct.length - 16);
        return { blob, iv, tag };
    },

    /* Sign uuid(16) || version_be(8) || blob || iv || tag — the exact message
       the server reconstructs and verifies. */
    async signSync(documentId, version, blob, iv, tag) {
        const uuidBytes = Uint8Array.from(documentId.replace(/-/g, '').match(/.{2}/g).map((h) => parseInt(h, 16)));
        const ver = new Uint8Array(8);
        new DataView(ver.buffer).setBigInt64(0, BigInt(version));
        const msg = new Uint8Array(16 + 8 + blob.length + iv.length + tag.length);
        msg.set(uuidBytes, 0); msg.set(ver, 16); msg.set(blob, 24);
        msg.set(iv, 24 + blob.length); msg.set(tag, 24 + blob.length + iv.length);
        const sig = new Uint8Array(await crypto.subtle.sign({ name: 'ECDSA', hash: 'SHA-256' }, this.signKey, msg));
        return b64.enc(p1363ToDer(sig));
    },

    toB64: b64.enc,
};

let sessionToken = null;
let online = false;
const listeners = new Set();

export function onConnection(fn) { listeners.add(fn); fn(status()); }
export function status() { return { online, authed: !!sessionToken }; }
function emit() { const s = status(); listeners.forEach((fn) => fn(s)); }

async function raw(method, path, body, { auth = true } = {}) {
    const headers = {};
    if (auth && sessionToken) {
        headers.Authorization = `Bearer ${sessionToken}`;
        headers['X-Device-Id'] = identity.deviceId;
    }
    const opts = { method, headers };
    if (body !== undefined) {
        headers['Content-Type'] = 'application/json';
        opts.body = JSON.stringify(body);
    }
    const res = await fetch(`${BASE}${path}`, opts);
    let data = null;
    try { data = await res.json(); } catch { /* empty body */ }
    return { status: res.status, data };
}

export const get = (p, o) => raw('GET', p, undefined, o);
export const post = (p, b, o) => raw('POST', p, b ?? {}, o);
export const del = (p, o) => raw('DELETE', p, undefined, o);

/* ── Session ────────────────────────────────────────────────────────────────
   The browser cannot perform Apple App Attest, so in development the backend
   exposes /auth/dev-session which registers this device id and mints a real
   session token. On iOS hardware the native app runs the true attestation
   flow against the same endpoints. */
export async function connect() {
    try {
        await deviceCrypto.init();
        const ping = await get('/auth/challenge', { auth: false });
        online = ping.status === 200;
        if (!online) { emit(); return false; }
        const res = await post(
            '/auth/dev-session',
            { device_id: identity.deviceId, public_key: deviceCrypto.publicKeyB64 },
            { auth: false },
        );
        if (res.status === 200 && res.data?.token) {
            sessionToken = res.data.token;
        }
    } catch {
        online = false;
    }
    emit();
    return online && !!sessionToken;
}

/* ── Account identity layer ──────────────────────────────────────────────────
   Sign-in / sign-up sits ON TOP of the device session: each account endpoint
   registers this device (with its real WebCrypto public key) and mints the
   same device-bound token dev-session would, then attaches the account. Email
   / password uses server-side PBKDF2; Apple / Google use OpenID Connect. The
   browser cannot run the native Apple/Google SDKs (and the CSP forbids loading
   them), so in this web build the social buttons present a simulated id-token
   (`sim:<subject>:<email>`) the backend accepts in development — the same
   account round-trips every time because the subject is stored per provider. */
const LS_ACCOUNT = 'lifeline.account';
const LS_OAUTH_SUB = 'lifeline.oauth_sub';

async function accountCall(path, body) {
    await deviceCrypto.init();
    const res = await post(
        path,
        { ...body, device_id: identity.deviceId, public_key: deviceCrypto.publicKeyB64 },
        { auth: false },
    );
    if (res.status === 200 && res.data?.token) {
        sessionToken = res.data.token;
        online = true;
        if (res.data.account) {
            localStorage.setItem(LS_ACCOUNT, JSON.stringify(res.data.account));
        }
        emit();
    }
    return res;
}

/* A stable per-provider subject so "Continue with Apple/Google" resolves to the
   same simulated account across sessions on this device. */
function oauthSubject(provider) {
    const store = JSON.parse(localStorage.getItem(LS_OAUTH_SUB) || '{}');
    if (!store[provider]) {
        store[provider] = `${provider}_${crypto.randomUUID()}`;
        localStorage.setItem(LS_OAUTH_SUB, JSON.stringify(store));
    }
    return store[provider];
}

export const account = {
    get current() {
        try { return JSON.parse(localStorage.getItem(LS_ACCOUNT) || 'null'); }
        catch { return null; }
    },
    register: (email, password) => accountCall('/account/register', { email, password }),
    login: (email, password) => accountCall('/account/login', { email, password }),
    /* provider: "apple" | "google". Builds the dev-accepted simulated token
       from a stable subject + the user's email so the flow is exercisable
       without live OAuth client credentials in the browser. */
    social: (provider, email) => {
        const subject = oauthSubject(provider);
        const id_token = `sim:${subject}:${email || ''}`;
        return accountCall('/account/oauth', { provider, id_token });
    },
    signOut() {
        localStorage.removeItem(LS_ACCOUNT);
        sessionToken = null;
        emit();
    },
};

/* Refresh the token before it can expire (dev TTL is 1h). */
export function keepAlive(intervalMs = 20 * 60 * 1000) {
    setInterval(() => { connect(); }, intervalMs);
}

/* ── Typed endpoints ──────────────────────────────────────────────────────── */
export const api = {
    insightsConfig: () => get('/insights/config', { auth: false }),
    gameConfig: () => get('/game/config', { auth: false }),
    billingConfig: () => get('/billing/config', { auth: false }),

    gameProfile: () => get('/game/profile'),
    submitScore: (vitality_score, handle) =>
        post('/game/score', handle ? { vitality_score, handle } : { vitality_score }),
    leaderboard: (limit = 50) => get(`/game/leaderboard?limit=${limit}`),

    subscription: () => get('/billing/subscription'),
    checkout: (tier) => post('/billing/checkout', { tier }),
    portal: () => post('/billing/portal'),
    donate: (amount_usd_cents) => post('/billing/donate', { amount_usd_cents }),
    storeReceipt: (platform, tier, receipt) =>
        post('/billing/store-receipt', { platform, tier, receipt }),
    betaFeatures: () => get('/billing/beta-features'),

    integrations: () => get('/integrations'),
    connectProvider: (p) => post(`/integrations/${p}/connect`, { authorized: true }),
    disconnectProvider: (p) => del(`/integrations/${p}`),
    whoopAuthorize: () => get('/integrations/whoop/authorize'),
    whoopCallback: (query) => get(`/integrations/whoop/callback${query}`, { auth: false }),
    whoopMetrics: () => get('/integrations/whoop/metrics'),

    syncDelta: (doc) => post('/sync/delta', doc),
    documentsByType: (t) => get(`/sync/documents/${t}`),

    challenge: () => get('/auth/challenge', { auth: false }),
    aiProxy: (prompt, execution_token) => post('/ai/proxy', { prompt, execution_token }),

    health: () => fetch('/health').then((r) => r.json()).catch(() => null),
};
