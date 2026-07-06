import Foundation
import Network
import NetworkExtension
import SystemConfiguration.CaptiveNetwork
import CLingXiaRustAPI
import os.log

#if os(iOS)
import UIKit
#elseif os(macOS)
import CoreWLAN
#endif

/**
 * WiFi management for iOS and macOS
 *
 * Platform Limitations:
 * iOS:
 * - Cannot scan for WiFi networks (privacy restriction)
 * - Can only connect to hotspot networks via NEHotspotConfiguration
 * - Getting connected WiFi info requires location permission + entitlements
 * - iOS 13+ deprecated CNCopyCurrentNetworkInfo
 *
 * macOS:
 * - Full WiFi scanning and connection support via CoreWLAN
 * - Can get detailed network information including signal strength
 */
class LxAppWifi {

    private static let log = OSLog(subsystem: "LingXia", category: "WiFi")

    // Multi-LxApp support: maintain a set of state listeners
    private nonisolated(unsafe) static var stateCallbacks: Set<UInt64> = []
    private nonisolated(unsafe) static var wifiPathMonitor: NWPathMonitor? = nil
    private static let wifiPathQueue = DispatchQueue(label: "LingXia.WifiMonitor")
    private nonisolated(unsafe) static var lastConnectedSignature: String? = nil
    private static let signatureLock = NSLock()

    /**
     * Initialize WiFi module
     * On iOS/macOS, no explicit initialization is needed
     */
    nonisolated public static func startWifi(callback_id: UInt64) {
        #if os(iOS) || os(macOS)
        // Request location permission (required for WiFi info on Apple platforms)
        Task { @MainActor in
            PermissionManager.ensureLocationWhenInUseAccess { granted in
                guard granted else {
                    os_log("%@", log: Self.log, type: .info, "WiFi module initialization denied due to missing location permission")
                    let _ = onCallback(callback_id, false, "12006") // Permission denied
                    return
                }

                os_log("%@", log: Self.log, type: .info, "WiFi module initialized with location permission")
                let _ = onCallback(callback_id, true, "{}")
            }
        }
        #else
        // macOS doesn't need location permission
        os_log("%@", log: Self.log, type: .info, "WiFi module initialized")
        let _ = onCallback(callback_id, true, "{}")
        #endif
    }

    /**
     * Stop WiFi module
     */
    nonisolated public static func stopWifi(callback_id: UInt64) {
        os_log("%@", log: Self.log, type: .info, "WiFi module stopped")
        let _ = onCallback(callback_id, true, "{}")
    }

    /**
     * Add a listener for WiFi connection state changes
     * Multiple listeners can be registered (supports multiple LxApp instances)
     */
    nonisolated public static func addWifiStateListener(callback_id: UInt64) {
        os_log("%@", log: Self.log, type: .info, "addWifiStateListener: callbackId=\(callback_id)")

        #if os(iOS) || os(macOS)
        // Check location permission (should already be granted by startWifi)
        // This is a defensive check - permission should have been requested during startWifi
        Task { @MainActor in
            PermissionManager.ensureLocationWhenInUseAccess { granted in
                guard granted else {
                    os_log("%@", log: Self.log, type: .info, "Location permission not granted - skip WiFi listener registration")
                    return
                }

                registerWifiStateListener(callback_id)
            }
        }
        #else
        // macOS doesn't need location permission
        registerWifiStateListener(callback_id)
        #endif
    }

    private static func registerWifiStateListener(_ callback_id: UInt64) {
        if stateCallbacks.insert(callback_id).inserted {
            os_log("%@", log: Self.log, type: .info, "Added WiFi state listener: \(callback_id) (total=\(stateCallbacks.count))")

            // First subscriber: start system WiFi monitoring
            if stateCallbacks.count == 1 {
                lastConnectedSignature = nil
                startWifiMonitor()
            }

            // Send current state to new subscriber
            emitWifiConnectedToCallback(callback_id)
        } else {
            LXLog.warn("WiFi state listener already exists: \(callback_id)", category: "WiFi")
        }
    }

    /**
     * Remove a previously registered WiFi state listener
     */
    nonisolated public static func removeWifiStateListener(callback_id: UInt64) {
        os_log("%@", log: Self.log, type: .info, "removeWifiStateListener: callbackId=\(callback_id)")

        if stateCallbacks.remove(callback_id) != nil {
            os_log("%@", log: Self.log, type: .info, "Removed WiFi state listener: \(callback_id) (remaining=\(stateCallbacks.count))")

            // Last subscriber: stop system WiFi monitoring
            if stateCallbacks.isEmpty {
                stopWifiMonitor()
                lastConnectedSignature = nil
            }
        } else {
            LXLog.warn("WiFi state listener not found: \(callback_id)", category: "WiFi")
        }
    }

    /**
     * Connect to WiFi access point
     * iOS: Only works for hotspot networks via NEHotspotConfiguration
     * macOS: Uses CoreWLAN to connect to networks
     */
    nonisolated public static func connectWifi(
        callback_id: UInt64,
        ssid: RustStr,
        password: RustStr?
    ) {
        // Convert RustStr to String
        let ssidString = ssid.toString()
        let passwordString: String?
        if let password = password, password.start != nil {
            passwordString = password.toString()
        } else {
            passwordString = nil
        }

        #if os(iOS)
        guard #available(iOS 11.0, *) else {
            LXLog.error("NEHotspotConfiguration requires iOS 11+", category: "WiFi")
            let _ = onCallback(callback_id, false, "12005") // Not supported
            return
        }

        // Create hotspot configuration
        let configuration: NEHotspotConfiguration

        if let passwordString = passwordString, !passwordString.isEmpty {
            // WPA/WPA2 Personal
            configuration = NEHotspotConfiguration(ssid: ssidString, passphrase: passwordString, isWEP: false)
        } else {
            // Open network
            configuration = NEHotspotConfiguration(ssid: ssidString)
        }

        configuration.joinOnce = false // Remember network

        // Apply configuration
        NEHotspotConfigurationManager.shared.apply(configuration) { error in
            if let error = error {
                LXLog.error("Failed to connect to WiFi: \(error.localizedDescription)", category: "WiFi")

                // Map error codes
                let errorCode: String
                switch (error as NSError).code {
                case NEHotspotConfigurationError.invalid.rawValue:
                    errorCode = "12002" // Password error or invalid config
                case NEHotspotConfigurationError.alreadyAssociated.rawValue:
                    os_log("%@", log: Self.log, type: .info, "Already connected to network")
                    emitWifiConnectedToAll(
                        connected: true,
                        ssid: ssidString,
                        bssid: nil,
                        secure: nil,  // iOS doesn't provide real security info
                        signalStrength: nil  // iOS doesn't provide real signal strength
                    )
                    let _ = onCallback(callback_id, true, "{}")
                    return
                case NEHotspotConfigurationError.userDenied.rawValue:
                    errorCode = "12006" // Permission denied
                default:
                    errorCode = "12001" // System error
                }

                let _ = onCallback(callback_id, false, errorCode)
                return
            }

            os_log("%@", log: Self.log, type: .info, "Successfully connected to WiFi: \(ssidString)")
            let _ = onCallback(callback_id, true, "{}")
            emitWifiConnectedToAll(
                connected: true,
                ssid: ssidString,
                bssid: nil,
                secure: nil,  // iOS doesn't provide real security info
                signalStrength: nil  // iOS doesn't provide real signal strength
            )
        }
        #elseif os(macOS)
        let client = CWWiFiClient.shared()

        guard let interface = client.interface() else {
            LXLog.error("No WiFi interface available", category: "WiFi")
            let _ = onCallback(callback_id, false, "12001") // System error
            return
        }

        // Scan for the target network
        do {
            let networks = try interface.scanForNetworks(withSSID: nil)

            // Find the target network by SSID
            guard let targetNetwork = networks.first(where: { network in
                network.ssid == ssidString
            }) else {
            LXLog.error("Network not found: \(ssidString)", category: "WiFi")
            let _ = onCallback(callback_id, false, "12010") // Network not found
            return
        }

            // Connect to the network
            if let passwordString = passwordString, !passwordString.isEmpty {
                try interface.associate(to: targetNetwork, password: passwordString)
            } else {
                try interface.associate(to: targetNetwork, password: nil)
            }

            os_log("%@", log: Self.log, type: .info, "Successfully connected to WiFi: \(ssidString)")
            let _ = onCallback(callback_id, true, "{}")
            emitWifiConnectedToAll(
                connected: true,
                ssid: ssidString,
                bssid: interface.bssid(),
                secure: !targetNetwork.supportsSecurity(.none),
                signalStrength: rssiToStrength(interface.rssiValue())
            )
        } catch {
            LXLog.error("Failed to connect to WiFi: \(error.localizedDescription)", category: "WiFi")
            let _ = onCallback(callback_id, false, "12002") // Connection error
        }
        #endif
    }

    /**
     * Get WiFi list (scan results)
     * iOS: Not supported - privacy restriction
     * macOS: Full scanning support via CoreWLAN
     */
    nonisolated public static func getWifiList(callback_id: UInt64) {
        #if os(iOS)
        LXLog.error("WiFi scanning not supported on iOS (platform limitation)", category: "WiFi")
        let _ = onCallback(callback_id, false, "12005") // Not supported
        #elseif os(macOS)
        let client = CWWiFiClient.shared()

        guard let interface = client.interface() else {
            LXLog.error("No WiFi interface available", category: "WiFi")
            let _ = onCallback(callback_id, false, "12001") // System error
            return
        }

        // Scan for networks
        do {
            let networks = try interface.scanForNetworks(withSSID: nil)

            var wifiList: [[String: Any]] = []

            for network in networks {
                guard let ssid = network.ssid, !ssid.isEmpty else {
                    continue
                }

                let signalStrength = rssiToStrength(network.rssiValue)

                var wifiInfo: [String: Any] = [
                    "ssid": ssid,
                    "secure": !network.supportsSecurity(.none),
                    "signalStrength": max(0, min(100, signalStrength))
                ]

                if let bssid = network.bssid {
                    wifiInfo["bssid"] = bssid
                }

                wifiList.append(wifiInfo)
            }

            // Serialize to JSON
            if let jsonData = try? JSONSerialization.data(withJSONObject: wifiList, options: []),
               let jsonString = String(data: jsonData, encoding: .utf8) {
                os_log("%@", log: Self.log, type: .info, "Found \(wifiList.count) WiFi networks")
                let _ = onCallback(callback_id, true, jsonString)
            } else {
                LXLog.error("Failed to serialize WiFi list", category: "WiFi")
                let _ = onCallback(callback_id, false, "12001") // System error
            }
        } catch {
            LXLog.error("Failed to scan WiFi networks: \(error.localizedDescription)", category: "WiFi")
            let _ = onCallback(callback_id, false, "12001") // System error
        }
        #endif
    }

    /**
     * Check if WiFi is enabled (synchronous)
     * iOS: WiFi state is not directly accessible, always returns true
     * macOS: Checks if WiFi interface is powered on
     */
    nonisolated public static func isWifiEnabled() -> Bool {
        #if os(iOS)
        // iOS doesn't provide an API to check WiFi state
        // Assume WiFi is available if we can access network interfaces
        return true
        #elseif os(macOS)
        let client = CWWiFiClient.shared()
        guard let interface = client.interface() else {
            return false
        }
        return interface.powerOn()
        #else
        return false
        #endif
    }

    /**
     * Get connected WiFi info
     * iOS: Requires location permission and proper entitlements (iOS 13+)
     * macOS: Full WiFi info available via CoreWLAN
     */
    nonisolated public static func getConnectedWifi(callback_id: UInt64) {
        #if os(iOS)
        Task { @MainActor in
            PermissionManager.ensureLocationWhenInUseAccess { granted in
                guard granted else {
                    LXLog.error("Location permission denied", category: "WiFi")
                    let _ = onCallback(callback_id, false, "12006") // Permission denied
                    return
                }

                // Try to get WiFi info using CNCopyCurrentNetworkInfo
                // Note: This API is deprecated in iOS 13+ and requires:
                // - Location permission
                // - Access WiFi Information entitlement
                // - App must be in foreground
                guard let interfaces = CNCopySupportedInterfaces() as? [String] else {
                    LXLog.error("No WiFi interfaces found", category: "WiFi")
                    let _ = onCallback(callback_id, false, "12001") // System error
                    return
                }

                for interface in interfaces {
                    guard let networkInfo = CNCopyCurrentNetworkInfo(interface as CFString) as? [String: Any] else {
                        continue
                    }

                    // Extract SSID and BSSID
                    guard let ssid = networkInfo[kCNNetworkInfoKeySSID as String] as? String else {
                        continue
                    }

                    let bssid = networkInfo[kCNNetworkInfoKeyBSSID as String] as? String

                    // Build result - only include fields we actually know
                    var result: [String: Any] = [
                        "ssid": ssid,
                    ]

                    if let bssid = bssid {
                        result["bssid"] = bssid
                    }

                    // iOS CNCopyCurrentNetworkInfo doesn't provide:
                    // - signal strength (rssi)
                    // - security type
                    // - frequency
                    // Don't include fake/default values for unknown fields

                    // Serialize to JSON
                    if let jsonData = try? JSONSerialization.data(withJSONObject: result, options: []),
                       let jsonString = String(data: jsonData, encoding: .utf8) {
                        os_log("%@", log: Self.log, type: .info, "Connected WiFi: \(ssid)")
                        let _ = onCallback(callback_id, true, jsonString)
                        return
                    }
                }

                // No connected WiFi found (or insufficient entitlement on iOS)
                os_log("%@", log: Self.log, type: .info, "No WiFi connected or permission denied")
                let _ = onCallback(callback_id, false, "12001") // System error
            }
        }
        #elseif os(macOS)
        Task { @MainActor in
            PermissionManager.ensureLocationWhenInUseAccess { granted in
                guard granted else {
                    os_log("%@", log: Self.log, type: .info, "Location permission denied on macOS")
                    let _ = onCallback(callback_id, false, "12006") // Permission denied
                    return
                }

                let client = CWWiFiClient.shared()

                guard let interface = client.interface() else {
                    LXLog.error("No WiFi interface available", category: "WiFi")
                    let _ = onCallback(callback_id, false, "12001") // System error
                    return
                }

                guard let ssid = interface.ssid(), !ssid.isEmpty else {
                    os_log("%@", log: Self.log, type: .info, "No WiFi connected")
                    let _ = onCallback(callback_id, false, "12001") // Not connected
                    return
                }

                var result: [String: Any] = [
                    "ssid": ssid
                ]

                if let bssid = interface.bssid() {
                    result["bssid"] = bssid
                }

                // Get security type
                let secure = interface.security() != .none
                result["secure"] = secure

                // Convert RSSI to signal strength (0-100)
                let signalStrength = rssiToStrength(interface.rssiValue())
                result["signalStrength"] = max(0, min(100, signalStrength))

                // Serialize to JSON
                if let jsonData = try? JSONSerialization.data(withJSONObject: result, options: []),
                   let jsonString = String(data: jsonData, encoding: .utf8) {
                    os_log("%@", log: Self.log, type: .info, "Connected WiFi: \(ssid)")
                    let _ = onCallback(callback_id, true, jsonString)
                } else {
                    LXLog.error("Failed to serialize WiFi info", category: "WiFi")
                    let _ = onCallback(callback_id, false, "12001") // System error
                }
            }
        }
        #endif
    }

    private static func rssiToStrength(_ rssi: Int) -> Int {
        if rssi >= -30 {
            return 100
        }
        if rssi <= -100 {
            return 0
        }
        return Int((Double(rssi + 100) / 70.0) * 100.0)
    }

    /**
     * Build WiFi info JSON with deduplication
     */
    private static func buildWifiInfoJson(
        connected: Bool,
        ssid: String,
        bssid: String?,
        secure: Bool?,
        signalStrength: Int?
    ) -> String? {
        let signature = "\(connected ? 1 : 0)|\(ssid)|\(bssid ?? "")"

        LxAppWifi.signatureLock.lock()
        if signature == lastConnectedSignature {
            LxAppWifi.signatureLock.unlock()
            return nil
        }
        lastConnectedSignature = signature
        let skipInitial = !connected && ssid.isEmpty && lastConnectedSignature == "0||"
        LxAppWifi.signatureLock.unlock()
        if skipInitial {
            return nil
        }

        var payload: [String: Any] = [
            "ssid": ssid,
            "connected": connected,
            "state": connected ? "connected" : "disconnected"
        ]

        // Only include optional fields if they have real values
        if let bssid = bssid {
            payload["bssid"] = bssid
        }
        if let secure = secure {
            payload["secure"] = secure
        }
        if let signalStrength = signalStrength {
            let normalizedSignalStrength = connected ? max(0, min(100, signalStrength)) : 0
            payload["signalStrength"] = normalizedSignalStrength
        }

        guard let jsonData = try? JSONSerialization.data(withJSONObject: payload, options: []),
              let jsonString = String(data: jsonData, encoding: .utf8) else {
            return nil
        }

        return jsonString
    }

    /**
     * Emit WiFi connected event to a specific callback
     */
    private static func emitWifiConnected(
        callbackId: UInt64,
        connected: Bool,
        ssid: String,
        bssid: String?,
        secure: Bool?,
        signalStrength: Int?
    ) {
        guard let jsonString = buildWifiInfoJson(
            connected: connected,
            ssid: ssid,
            bssid: bssid,
            secure: secure,
            signalStrength: signalStrength
        ) else {
            return
        }

        os_log("%@", log: Self.log, type: .info, "emitWifiConnected: callbackId=\(callbackId)")
        let success = onCallback(callbackId, true, jsonString)
        if !success {
            LXLog.warn("Failed to dispatch wifi connected event to callback \(callbackId)", category: "WiFi")
        }
    }

    /**
     * Broadcast WiFi connected event to all subscribers
     */
    private static func emitWifiConnectedToAll(
        connected: Bool,
        ssid: String,
        bssid: String?,
        secure: Bool?,
        signalStrength: Int?
    ) {
        if stateCallbacks.isEmpty {
            return
        }

        guard let jsonString = buildWifiInfoJson(
            connected: connected,
            ssid: ssid,
            bssid: bssid,
            secure: secure,
            signalStrength: signalStrength
        ) else {
            return
        }

        os_log("%@", log: Self.log, type: .info, "emitWifiConnectedToAll: \(stateCallbacks.count) subscribers")
        for callbackId in stateCallbacks {
            let success = onCallback(callbackId, true, jsonString)
            if !success {
                LXLog.warn("Failed to dispatch wifi connected event to callback \(callbackId)", category: "WiFi")
            }
        }
    }

    /**
     * Emit current WiFi state to a specific callback
     */
    private static func emitWifiConnectedToCallback(_ callbackId: UInt64) {
        if let info = currentWifiInfo() {
            emitWifiConnected(
                callbackId: callbackId,
                connected: true,
                ssid: info.ssid,
                bssid: info.bssid,
                secure: info.secure,
                signalStrength: info.signalStrength
            )
            return
        }
        emitWifiConnected(
            callbackId: callbackId,
            connected: false,
            ssid: "",
            bssid: nil,
            secure: false,
            signalStrength: 0
        )
    }

    /**
     * Emit current WiFi state to all subscribers
     */
    private static func emitWifiConnectedFromCurrent() {
        if let info = currentWifiInfo() {
            emitWifiConnectedToAll(
                connected: true,
                ssid: info.ssid,
                bssid: info.bssid,
                secure: info.secure,
                signalStrength: info.signalStrength
            )
            return
        }
        emitWifiConnectedToAll(
            connected: false,
            ssid: "",
            bssid: nil,
            secure: false,
            signalStrength: 0
        )
    }

    private static func startWifiMonitor() {
        #if os(iOS)
        if #available(iOS 12.0, *) {
            if wifiPathMonitor != nil {
                return
            }
            let monitor = NWPathMonitor(requiredInterfaceType: .wifi)
            monitor.pathUpdateHandler = { _ in
                emitWifiConnectedFromCurrent()
            }
            wifiPathMonitor = monitor
            monitor.start(queue: wifiPathQueue)
        }
        #elseif os(macOS)
        if #available(macOS 10.14, *) {
            if wifiPathMonitor != nil {
                return
            }
            let monitor = NWPathMonitor(requiredInterfaceType: .wifi)
            monitor.pathUpdateHandler = { _ in
                emitWifiConnectedFromCurrent()
            }
            wifiPathMonitor = monitor
            monitor.start(queue: wifiPathQueue)
        }
        #endif
    }

    private static func stopWifiMonitor() {
        wifiPathMonitor?.cancel()
        wifiPathMonitor = nil
    }

    private static func currentWifiInfo() -> (ssid: String, bssid: String?, secure: Bool?, signalStrength: Int?)? {
        #if os(iOS)
        guard let interfaces = CNCopySupportedInterfaces() as? [String] else {
            return nil
        }
        for interface in interfaces {
            guard let networkInfo = CNCopyCurrentNetworkInfo(interface as CFString) as? [String: Any],
                  let ssid = networkInfo[kCNNetworkInfoKeySSID as String] as? String else {
                continue
            }
            let bssid = networkInfo[kCNNetworkInfoKeyBSSID as String] as? String
            // iOS CNCopyCurrentNetworkInfo doesn't provide secure or signal strength - return nil for unknown values
            return (ssid: ssid, bssid: bssid, secure: nil, signalStrength: nil)
        }
        return nil
        #elseif os(macOS)
        let client = CWWiFiClient.shared()
        guard let interface = client.interface() else {
            return nil
        }
        guard let ssid = interface.ssid(), !ssid.isEmpty else {
            return nil
        }
        let bssid = interface.bssid()
        let secure = interface.security() != .none
        let signalStrength = rssiToStrength(interface.rssiValue())
        return (ssid: ssid, bssid: bssid, secure: secure, signalStrength: signalStrength)
        #else
        return nil
        #endif
    }
}
