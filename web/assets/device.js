/* Lifeline device capability scanner.
   Reads what this device can actually do — platform, memory, cores, GPU, and
   whether a real AI-inference backend is present — and reduces it to a tier
   (`entry` / `capable` / `premium`) plus an `onDeviceAiEligible` flag. This is
   what decides whether we offer to run the coach fully on-device (Gemma) so
   the app can work with no internet at all.

   Everything here is read locally from the browser/OS. Nothing is sent to the
   server; the scan result stays on the device. */

/* Native shells (Capacitor) may expose a real on-device inference bridge and
   richer device info than a browser can see. */
const CAP = typeof window !== 'undefined' ? window.Capacitor : undefined;
const IN_SHELL = typeof CAP !== 'undefined';

/* A native app can inject a hardware profile the browser can't read (exact RAM,
   chipset, Neural Engine presence). Shape: { ram_gb, cores, chipset, os,
   os_version, has_npu, ai_backends: ["native-mediapipe", ...] }. */
function nativeProfile() {
    try {
        return (IN_SHELL && window.LifelineDevice && window.LifelineDevice.profile) || null;
    } catch { return null; }
}

/* Which inference backends this device exposes, best-first. A native runtime
   bridge (MediaPipe LLM / Core ML) is preferred; WebGPU is the browser path. */
function inferenceBackends() {
    const out = [];
    const np = nativeProfile();
    if (np && Array.isArray(np.ai_backends)) out.push(...np.ai_backends);
    // A native shell that advertises a local-AI bridge can run models even if
    // it didn't spell out backends in its profile.
    if (IN_SHELL && typeof window.LifelineLocalAI !== 'undefined' && !out.length) {
        out.push('native-bridge');
    }
    // WebGPU: the browser-side path (WebLLM / transformers.js). Feature-detected.
    if (typeof navigator !== 'undefined' && 'gpu' in navigator) out.push('webgpu');
    return [...new Set(out)];
}

function detectPlatform() {
    const np = nativeProfile();
    if (np?.os) return np.os;
    const ua = (navigator.userAgent || '').toLowerCase();
    const plat = (navigator.platform || '').toLowerCase();
    if (/iphone|ipad|ipod/.test(ua) || (plat === 'macintel' && navigator.maxTouchPoints > 1)) return 'ios';
    if (/android/.test(ua)) return 'android';
    if (/mac/.test(plat)) return 'macos';
    if (/win/.test(plat)) return 'windows';
    if (/linux/.test(plat)) return 'linux';
    return 'unknown';
}

/* Best-effort RAM in GB. `navigator.deviceMemory` is capped at 8 and coarse,
   and absent on iOS Safari — a native profile is far more accurate when present. */
function detectRamGb() {
    const np = nativeProfile();
    if (np?.ram_gb) return { value: np.ram_gb, exact: true };
    if (typeof navigator.deviceMemory === 'number') {
        return { value: navigator.deviceMemory, exact: false };
    }
    return { value: null, exact: false };
}

function detectCores() {
    const np = nativeProfile();
    if (np?.cores) return np.cores;
    return typeof navigator.hardwareConcurrency === 'number' ? navigator.hardwareConcurrency : null;
}

/* Persisted storage estimate — a model download needs headroom. */
async function storageGb() {
    try {
        if (navigator.storage?.estimate) {
            const { quota } = await navigator.storage.estimate();
            if (quota) return Math.round((quota / 1e9) * 10) / 10;
        }
    } catch { /* not available */ }
    return null;
}

/* Reduce the raw signals to a coarse tier. Deliberately conservative: unknown
   memory never counts as premium. Cores + a modern GPU/NPU move a device up. */
function computeTier({ ramGb, cores, backends, hasNpu }) {
    const ram = ramGb ?? 0;
    const c = cores ?? 0;
    const strongBackend = backends.some((b) => b.startsWith('native-')) || hasNpu;
    // Premium: enough RAM for a 2B model and a real accelerator.
    if (ram >= 6 && c >= 6 && (strongBackend || backends.includes('webgpu'))) return 'premium';
    // deviceMemory caps at 8 and is coarse; treat 8 + WebGPU as premium too,
    // since that's a high-end phone or a real computer.
    if (ram >= 8 && backends.includes('webgpu')) return 'premium';
    // Capable: can likely run a 1B model.
    if (ram >= 4 && c >= 4 && backends.length) return 'capable';
    return 'entry';
}

let cached = null;

/* Run the scan once and memoize. Returns a plain object safe to render. */
export async function scanDevice() {
    if (cached) return cached;
    const np = nativeProfile();
    const platform = detectPlatform();
    const ram = detectRamGb();
    const cores = detectCores();
    const backends = inferenceBackends();
    const hasNpu = !!np?.has_npu;
    const storage = await storageGb();
    const tier = computeTier({ ramGb: ram.value, cores, backends, hasNpu });

    cached = {
        platform,
        inShell: IN_SHELL,
        osVersion: np?.os_version || null,
        chipset: np?.chipset || null,
        ramGb: ram.value,
        ramExact: ram.exact,
        cores,
        storageGb: storage,
        hasNpu,
        backends,
        tier,
        // A device is eligible to run the coach locally if it clears the
        // capable bar AND exposes at least one inference backend.
        onDeviceAiEligible: (tier === 'capable' || tier === 'premium') && backends.length > 0,
        scannedAt: new Date().toISOString(),
    };
    return cached;
}

/* Cross-check a scan against the server's model-catalog eligibility floor.
   Returns the subset of models this device can actually run, best-first. */
export function eligibleModels(scan, catalog) {
    if (!scan?.onDeviceAiEligible || !catalog?.models) return [];
    const floor = catalog.eligibility || {};
    if (floor.min_ram_gb && scan.ramGb != null && scan.ramGb < floor.min_ram_gb) return [];
    if (floor.min_cpu_cores && scan.cores != null && scan.cores < floor.min_cpu_cores) return [];
    const tierRank = { entry: 0, capable: 1, premium: 2 };
    return catalog.models.filter((m) => {
        const tierOk = tierRank[scan.tier] >= tierRank[m.min_device_tier ?? 'premium'];
        const ramOk = scan.ramGb == null || scan.ramGb >= (m.min_ram_gb ?? 0);
        const backendOk = !m.backends || m.backends.some((b) => scan.backends.includes(b));
        return tierOk && ramOk && backendOk;
    });
}

/* A human label for the tier, for UI copy. */
export const TIER_LABEL = {
    entry: 'Entry device',
    capable: 'Capable device',
    premium: 'Premium device',
};
