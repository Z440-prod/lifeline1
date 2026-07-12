import Foundation
import Capacitor
import StoreKit
import HealthKit
import DeviceCheck
import AuthenticationServices
import UserNotifications
import CryptoKit

/// Native capabilities for Lifeline. Each method backs one `window.Lifeline*`
/// bridge in the web app (see web/assets/native-bridge.js).
///
/// Build notes:
///  • Capabilities to enable in Xcode: In-App Purchase, HealthKit, App Attest,
///    Sign in with Apple, Push/Local Notifications.
///  • Info.plist: NSHealthShareUsageDescription (required for HealthKit read).
///  • Product IDs must match the backend: health.lifeline.app.pro_monthly /
///    health.lifeline.app.elite_monthly.
///  • Google sign-in and MediaPipe on-device AI are optional pods (see podspec);
///    the methods below degrade to a clear error if the SDK isn't linked.
@objc(LifelineNativePlugin)
public class LifelineNativePlugin: CAPPlugin {

    private let productIDs: [String: String] = [
        "pro": "health.lifeline.app.pro_monthly",
        "elite": "health.lifeline.app.elite_monthly"
    ]
    private let dailyNotificationID = "lifeline.daily"

    // MARK: - In-App Purchase (StoreKit 2)

    @objc func purchase(_ call: CAPPluginCall) {
        guard let tier = call.getString("tier"), let productID = productIDs[tier] else {
            call.reject("Unknown tier"); return
        }
        Task {
            do {
                let products = try await Product.products(for: [productID])
                guard let product = products.first else { call.reject("Product not found"); return }
                let result = try await product.purchase()
                switch result {
                case .success(let verification):
                    // Send the signed JWS transaction to the backend, which
                    // verifies it and upserts the subscription tier.
                    let jws = verification.jwsRepresentation
                    if case .verified(let transaction) = verification { await transaction.finish() }
                    call.resolve(["platform": "apple", "receipt": jws])
                case .userCancelled:
                    call.reject("cancelled")
                case .pending:
                    call.reject("pending")
                @unknown default:
                    call.reject("unknown")
                }
            } catch {
                call.reject("Purchase failed: \(error.localizedDescription)")
            }
        }
    }

    // MARK: - Notifications (daily check-in)

    @objc func requestNotificationPermission(_ call: CAPPluginCall) {
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge]) { granted, _ in
            call.resolve(["granted": granted])
        }
    }

    @objc func scheduleDaily(_ call: CAPPluginCall) {
        let hour = call.getInt("hour") ?? 9
        let minute = call.getInt("minute") ?? 0
        let content = UNMutableNotificationContent()
        content.title = "Your Lifeline is ready"
        content.body = "Open Lifeline for today’s note."
        content.sound = .default
        var date = DateComponents(); date.hour = hour; date.minute = minute
        let trigger = UNCalendarNotificationTrigger(dateMatching: date, repeats: true)
        let request = UNNotificationRequest(identifier: dailyNotificationID, content: content, trigger: trigger)
        let center = UNUserNotificationCenter.current()
        center.removePendingNotificationRequests(withIdentifiers: [dailyNotificationID])
        center.add(request) { error in
            if let error = error { call.reject(error.localizedDescription) } else { call.resolve() }
        }
    }

    @objc func cancelDaily(_ call: CAPPluginCall) {
        UNUserNotificationCenter.current().removePendingNotificationRequests(withIdentifiers: [dailyNotificationID])
        call.resolve()
    }

    @objc func showNotification(_ call: CAPPluginCall) {
        let content = UNMutableNotificationContent()
        content.title = call.getString("title") ?? "Lifeline"
        content.body = call.getString("body") ?? ""
        content.sound = .default
        let request = UNNotificationRequest(identifier: UUID().uuidString, content: content,
                                            trigger: UNTimeIntervalNotificationTrigger(timeInterval: 1, repeats: false))
        UNUserNotificationCenter.current().add(request) { _ in call.resolve() }
    }

    // MARK: - Device profile

    @objc func deviceProfile(_ call: CAPPluginCall) {
        let ramGB = Double(ProcessInfo.processInfo.physicalMemory) / 1_073_741_824.0
        let cores = ProcessInfo.processInfo.processorCount
        var sysinfo = utsname(); uname(&sysinfo)
        let machine = withUnsafePointer(to: &sysinfo.machine) {
            $0.withMemoryRebound(to: CChar.self, capacity: 1) { String(validatingUTF8: $0) ?? "unknown" }
        }
        // Every modern iPhone has a Neural Engine; expose the on-device backends.
        call.resolve([
            "ram_gb": Int(ramGB.rounded()),
            "cores": cores,
            "chipset": machine,
            "os": "ios",
            "os_version": UIDevice.current.systemVersion,
            "has_npu": true,
            "ai_backends": ["native-coreml", "native-mediapipe"]
        ])
    }

    // MARK: - App Attest (optional hardening)

    @objc func attest(_ call: CAPPluginCall) {
        guard let challenge = call.getString("challenge") else { call.reject("Missing challenge"); return }
        let service = DCAppAttestService.shared
        guard service.isSupported else { call.reject("App Attest unsupported"); return }
        service.generateKey { keyId, error in
            if let error = error { call.reject(error.localizedDescription); return }
            guard let keyId = keyId else { call.reject("No key"); return }
            let hash = Data(SHA256.hash(data: Data(challenge.utf8)))
            service.attestKey(keyId, clientDataHash: hash) { attestation, error in
                if let error = error { call.reject(error.localizedDescription); return }
                call.resolve([
                    "keyId": keyId,
                    "attestation": attestation?.base64EncodedString() ?? ""
                ])
            }
        }
    }

    // MARK: - Sign in with Apple

    private var signInCall: CAPPluginCall?

    @objc func signInApple(_ call: CAPPluginCall) {
        signInCall = call
        let request = ASAuthorizationAppleIDProvider().createRequest()
        request.requestedScopes = [.email]
        let controller = ASAuthorizationController(authorizationRequests: [request])
        controller.delegate = self
        controller.presentationContextProvider = self
        DispatchQueue.main.async { controller.performRequests() }
    }

    @objc func signInGoogle(_ call: CAPPluginCall) {
        // Requires the GoogleSignIn pod + your client ID in Info.plist. When
        // linked, present GIDSignIn and resolve idToken:
        //   GIDSignIn.sharedInstance.signIn(withPresenting: vc) { result, error in
        //       call.resolve(["idToken": result?.user.idToken?.tokenString ?? ""]) }
        call.reject("Google sign-in not linked. Add the GoogleSignIn pod and wire GIDSignIn.")
    }

    // MARK: - HealthKit

    private let healthStore = HKHealthStore()

    @objc func requestHealthPermission(_ call: CAPPluginCall) {
        guard HKHealthStore.isHealthDataAvailable() else { call.resolve(["granted": false]); return }
        var read = Set<HKObjectType>()
        [HKQuantityTypeIdentifier.restingHeartRate, .heartRateVariabilitySDNN, .stepCount]
            .forEach { if let t = HKQuantityType.quantityType(forIdentifier: $0) { read.insert(t) } }
        if let sleep = HKObjectType.categoryType(forIdentifier: .sleepAnalysis) { read.insert(sleep) }
        healthStore.requestAuthorization(toShare: nil, read: read) { granted, _ in
            call.resolve(["granted": granted])
        }
    }

    @objc func readHealth(_ call: CAPPluginCall) {
        // Reads the latest resting HR, HRV, and today's steps and maps them into
        // the signal shape web/assets/engine.js expects. Sleep is left to a
        // fuller implementation; the web engine tolerates partial payloads.
        let group = DispatchGroup()
        var out: [String: Any] = [:]

        func latest(_ id: HKQuantityTypeIdentifier, unit: HKUnit, key: String) {
            guard let type = HKQuantityType.quantityType(forIdentifier: id) else { return }
            group.enter()
            let sort = NSSortDescriptor(key: HKSampleSortIdentifierEndDate, ascending: false)
            let q = HKSampleQuery(sampleType: type, predicate: nil, limit: 1, sortDescriptors: [sort]) { _, samples, _ in
                if let s = samples?.first as? HKQuantitySample {
                    out[key] = s.quantity.doubleValue(for: unit)
                }
                group.leave()
            }
            healthStore.execute(q)
        }

        func sumToday(_ id: HKQuantityTypeIdentifier, unit: HKUnit, key: String) {
            guard let type = HKQuantityType.quantityType(forIdentifier: id) else { return }
            group.enter()
            let start = Calendar.current.startOfDay(for: Date())
            let predicate = HKQuery.predicateForSamples(withStart: start, end: Date())
            let q = HKStatisticsQuery(quantityType: type, quantitySamplePredicate: predicate, options: .cumulativeSum) { _, stats, _ in
                if let sum = stats?.sumQuantity() { out[key] = sum.doubleValue(for: unit) }
                group.leave()
            }
            healthStore.execute(q)
        }

        latest(.restingHeartRate, unit: HKUnit(from: "count/min"), key: "resting_heart_rate")
        latest(.heartRateVariabilitySDNN, unit: HKUnit.secondUnit(with: .milli), key: "hrv_ms")
        sumToday(.stepCount, unit: .count(), key: "daily_steps")

        group.notify(queue: .main) { call.resolve(out as PluginCallResultData) }
    }

    // MARK: - On-device AI (MediaPipe LLM / Core ML)

    @objc func aiDownload(_ call: CAPPluginCall) {
        // With the MediaPipeTasksGenAI pod linked, download the model bundle for
        // the given modelId to the app's caches dir and emit progress via
        // notifyListeners("aiDownloadProgress", ["percent": pct]). Left as a
        // documented integration point so the plugin builds without the pod.
        call.reject("On-device AI not linked. Add MediaPipeTasksGenAI and implement model download.")
    }

    @objc func aiGenerate(_ call: CAPPluginCall) {
        call.reject("On-device AI not linked.")
    }

    @objc func aiRemove(_ call: CAPPluginCall) {
        call.resolve()
    }
}

// MARK: - Sign in with Apple delegates

extension LifelineNativePlugin: ASAuthorizationControllerDelegate, ASAuthorizationControllerPresentationContextProviding {
    public func authorizationController(controller: ASAuthorizationController, didCompleteWithAuthorization authorization: ASAuthorization) {
        guard let cred = authorization.credential as? ASAuthorizationAppleIDCredential,
              let tokenData = cred.identityToken,
              let idToken = String(data: tokenData, encoding: .utf8) else {
            signInCall?.reject("No identity token"); signInCall = nil; return
        }
        signInCall?.resolve(["idToken": idToken]); signInCall = nil
    }

    public func authorizationController(controller: ASAuthorizationController, didCompleteWithError error: Error) {
        signInCall?.reject("cancelled"); signInCall = nil
    }

    public func presentationAnchor(for controller: ASAuthorizationController) -> ASPresentationAnchor {
        return self.bridge?.viewController?.view.window ?? ASPresentationAnchor()
    }
}
