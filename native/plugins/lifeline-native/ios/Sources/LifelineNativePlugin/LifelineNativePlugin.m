#import <Foundation/Foundation.h>
#import <Capacitor/Capacitor.h>

// Registers the plugin + its methods with Capacitor's bridge. The method names
// and return types must match the @objc funcs in LifelineNativePlugin.swift.
CAP_PLUGIN(LifelineNativePlugin, "LifelineNative",
    CAP_PLUGIN_METHOD(purchase, CAPPluginReturnPromise);
    CAP_PLUGIN_METHOD(requestNotificationPermission, CAPPluginReturnPromise);
    CAP_PLUGIN_METHOD(scheduleDaily, CAPPluginReturnPromise);
    CAP_PLUGIN_METHOD(cancelDaily, CAPPluginReturnPromise);
    CAP_PLUGIN_METHOD(showNotification, CAPPluginReturnPromise);
    CAP_PLUGIN_METHOD(aiDownload, CAPPluginReturnPromise);
    CAP_PLUGIN_METHOD(aiGenerate, CAPPluginReturnPromise);
    CAP_PLUGIN_METHOD(aiRemove, CAPPluginReturnPromise);
    CAP_PLUGIN_METHOD(deviceProfile, CAPPluginReturnPromise);
    CAP_PLUGIN_METHOD(signInApple, CAPPluginReturnPromise);
    CAP_PLUGIN_METHOD(signInGoogle, CAPPluginReturnPromise);
    CAP_PLUGIN_METHOD(requestHealthPermission, CAPPluginReturnPromise);
    CAP_PLUGIN_METHOD(readHealth, CAPPluginReturnPromise);
    CAP_PLUGIN_METHOD(attest, CAPPluginReturnPromise);
)
