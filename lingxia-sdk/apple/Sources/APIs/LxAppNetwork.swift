import Foundation
import Network
#if canImport(CoreTelephony)
import CoreTelephony
#endif
import CLingXiaRustAPI
import Darwin

final class LxAppNetwork {
    private struct Status: Equatable {
        let isConnected: Bool
        let networkType: String
    }

    private static let initialWaitTimeoutSeconds: TimeInterval = 1.0
    private static let queue = DispatchQueue(label: "LingXia.Network.Monitor")
    private static let lock = NSLock()
    private nonisolated(unsafe) static var monitor: NWPathMonitor?
    private nonisolated(unsafe) static var callbacks: Set<UInt64> = []
    private nonisolated(unsafe) static var pendingNetworkInfoCallbacks: Set<UInt64> = []
    private nonisolated(unsafe) static var hasInitialStatus: Bool = false
    private nonisolated(unsafe) static var lastStatus: Status = .init(isConnected: false, networkType: "none")
    private nonisolated(unsafe) static var lastPreferredInterfaceNames: Set<String> = []
    private nonisolated(unsafe) static var lastNetworkInfoSignature: String = ""

    nonisolated static func getNetworkInfo(callback_id: UInt64) {
        ensureMonitor()

        lock.lock()
        let ready = hasInitialStatus
        let status = lastStatus
        let preferredInterfaceNames = lastPreferredInterfaceNames
        if !ready {
            pendingNetworkInfoCallbacks.insert(callback_id)
        }
        lock.unlock()

        if ready {
            emitNetworkInfoToCallback(
                callback_id,
                status: status,
                preferredInterfaceNames: preferredInterfaceNames
            )
            stopMonitorIfIdle()
        } else {
            scheduleNetworkInfoTimeout(for: callback_id)
        }
    }

    nonisolated static func addNetworkChangeListener(callback_id: UInt64) {
        ensureMonitor()

        lock.lock()
        callbacks.insert(callback_id)
        let ready = hasInitialStatus
        let status = lastStatus
        let preferredInterfaceNames = lastPreferredInterfaceNames
        lock.unlock()

        if ready {
            emitNetworkInfoToCallback(
                callback_id,
                status: status,
                preferredInterfaceNames: preferredInterfaceNames
            )
        }
    }

    nonisolated static func removeNetworkChangeListener(callback_id: UInt64) {
        lock.lock()
        callbacks.remove(callback_id)
        lock.unlock()

        stopMonitorIfIdle()
    }

    private static func ensureMonitor() {
        lock.lock()
        if monitor != nil {
            lock.unlock()
            return
        }

        let created = NWPathMonitor()
        monitor = created
        hasInitialStatus = false
        lastStatus = mapPath(created.currentPath)
        lastPreferredInterfaceNames = preferredInterfaceNames(for: created.currentPath)
        lastNetworkInfoSignature = ""
        lock.unlock()

        created.pathUpdateHandler = { path in
            let status = mapPath(path)
            let preferredInterfaceNames = preferredInterfaceNames(for: path)
            let localIPs: (ipv4: [String], ipv6: [String])
            if status.isConnected {
                localIPs = resolveLocalIPAddresses(preferredInterfaceNames: preferredInterfaceNames)
            } else {
                localIPs = (ipv4: [], ipv6: [])
            }
            let infoSignature = networkInfoSignature(status: status, addresses: localIPs)
            lock.lock()
            let previousSignature = lastNetworkInfoSignature
            lastStatus = status
            lastPreferredInterfaceNames = preferredInterfaceNames
            lastNetworkInfoSignature = infoSignature
            let ids = Array(callbacks)
            let pendingInfoIds = Array(pendingNetworkInfoCallbacks)
            pendingNetworkInfoCallbacks.removeAll()
            let firstUpdate = !hasInitialStatus
            hasInitialStatus = true
            lock.unlock()

            if firstUpdate || infoSignature != previousSignature {
                ids.forEach { id in
                    emitNetworkInfoToCallback(id, status: status, addresses: localIPs)
                }
            }

            pendingInfoIds.forEach { id in
                emitNetworkInfoToCallback(id, status: status, addresses: localIPs)
            }

            stopMonitorIfIdle()
        }

        created.start(queue: queue)
    }

    private static func stopMonitor() {
        lock.lock()
        let current = monitor
        monitor = nil
        pendingNetworkInfoCallbacks.removeAll()
        hasInitialStatus = false
        lastStatus = .init(isConnected: false, networkType: "none")
        lastPreferredInterfaceNames = []
        lastNetworkInfoSignature = ""
        lock.unlock()

        current?.cancel()
    }

    private static func stopMonitorIfIdle() {
        lock.lock()
        let shouldStop =
            monitor != nil &&
            callbacks.isEmpty &&
            pendingNetworkInfoCallbacks.isEmpty
        lock.unlock()
        if shouldStop {
            stopMonitor()
        }
    }

    private static func emitNetworkInfoToCallback(
        _ callbackId: UInt64,
        status: Status,
        preferredInterfaceNames: Set<String>
    ) {
        let addresses: (ipv4: [String], ipv6: [String])
        if status.isConnected {
            addresses = resolveLocalIPAddresses(preferredInterfaceNames: preferredInterfaceNames)
        } else {
            addresses = (ipv4: [], ipv6: [])
        }
        emitNetworkInfoToCallback(callbackId, status: status, addresses: addresses)
    }

    private static func emitNetworkInfoToCallback(
        _ callbackId: UInt64,
        status: Status,
        addresses: (ipv4: [String], ipv6: [String])
    ) {
        let connected = status.isConnected
        let ipv4 = connected ? addresses.ipv4 : [String]()
        let ipv6 = connected ? addresses.ipv6 : [String]()
        let payload: [String: Any] = [
            "isConnected": connected,
            "networkType": connected ? status.networkType : "none",
            "ipv4": ipv4,
            "ipv6": ipv6
        ]
        let json = jsonString(payload)
            ?? "{\"isConnected\":false,\"networkType\":\"none\",\"ipv4\":[],\"ipv6\":[]}"
        let _ = onCallback(callbackId, true, json)
    }

    private static func networkInfoSignature(
        status: Status,
        addresses: (ipv4: [String], ipv6: [String])
    ) -> String {
        return "\(status.isConnected):\(status.networkType):\(addresses.ipv4.joined(separator: ",")):\(addresses.ipv6.joined(separator: ","))"
    }

    private static func scheduleNetworkInfoTimeout(for callbackId: UInt64) {
        queue.asyncAfter(deadline: .now() + initialWaitTimeoutSeconds) {
            lock.lock()
            guard pendingNetworkInfoCallbacks.remove(callbackId) != nil else {
                lock.unlock()
                return
            }
            let status = lastStatus
            let preferredInterfaceNames = lastPreferredInterfaceNames
            lock.unlock()

            emitNetworkInfoToCallback(
                callbackId,
                status: status,
                preferredInterfaceNames: preferredInterfaceNames
            )
            stopMonitorIfIdle()
        }
    }

    private static func preferredInterfaceNames(for path: NWPath) -> Set<String> {
        var names: Set<String> = []

        let preferredTypes: [NWInterface.InterfaceType] = [.wifi, .cellular, .wiredEthernet]
        for interfaceType in preferredTypes where path.usesInterfaceType(interfaceType) {
            for iface in path.availableInterfaces where iface.type == interfaceType {
                names.insert(iface.name)
            }
        }
        if !names.isEmpty {
            return names
        }

        for iface in path.availableInterfaces where path.usesInterfaceType(iface.type) {
            names.insert(iface.name)
        }
        return names
    }

    private static func mapPath(_ path: NWPath) -> Status {
        if path.status != .satisfied {
            return .init(isConnected: false, networkType: "none")
        }
        if path.usesInterfaceType(.wifi) {
            return .init(isConnected: true, networkType: "wifi")
        }
        if path.usesInterfaceType(.cellular) {
            return .init(isConnected: true, networkType: resolveCellularNetworkType())
        }
        if path.usesInterfaceType(.wiredEthernet) {
            return .init(isConnected: true, networkType: "ethernet")
        }
        return .init(isConnected: true, networkType: "unknown")
    }

    private static func resolveCellularNetworkType() -> String {
        #if os(iOS)
        let info = CTTelephonyNetworkInfo()
        var tech: String?
        if #available(iOS 12.0, *) {
            tech = info.serviceCurrentRadioAccessTechnology?.values.first
        } else {
            tech = info.currentRadioAccessTechnology
        }
        guard let t = tech else {
            return "unknown"
        }

        // Map to 2g/3g/4g/5g when possible; otherwise return "unknown".
        if #available(iOS 14.1, *) {
            if t == CTRadioAccessTechnologyNR || t == CTRadioAccessTechnologyNRNSA {
                return "5g"
            }
        }

        switch t {
        case CTRadioAccessTechnologyLTE:
            return "4g"
        case CTRadioAccessTechnologyWCDMA,
             CTRadioAccessTechnologyHSDPA,
             CTRadioAccessTechnologyHSUPA,
             CTRadioAccessTechnologyCDMAEVDORev0,
             CTRadioAccessTechnologyCDMAEVDORevA,
             CTRadioAccessTechnologyCDMAEVDORevB,
             CTRadioAccessTechnologyeHRPD:
            return "3g"
        case CTRadioAccessTechnologyGPRS,
             CTRadioAccessTechnologyEdge,
             CTRadioAccessTechnologyCDMA1x:
            return "2g"
        default:
            return "unknown"
        }
        #else
        return "unknown"
        #endif
    }

    private static func jsonString(_ payload: [String: Any]) -> String? {
        guard let data = try? JSONSerialization.data(withJSONObject: payload, options: []) else {
            return nil
        }
        return String(data: data, encoding: .utf8)
    }

    private static func resolveLocalIPAddresses(
        preferredInterfaceNames: Set<String> = []
    ) -> (ipv4: [String], ipv6: [String]) {
        struct IPEntry {
            let interfaceName: String
            let family: sa_family_t
            let value: String
            let isLinkLocalV6: Bool
        }

        var entries: [IPEntry] = []

        var ifaddr: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&ifaddr) == 0, let firstAddr = ifaddr else {
            return ([], [])
        }
        defer { freeifaddrs(ifaddr) }

        func collectAddresses(
            from source: [IPEntry],
            includeLinkLocalIPv6: Bool
        ) -> (ipv4: [String], ipv6: [String]) {
            var ipv4: [String] = []
            var ipv6: [String] = []
            for entry in source {
                if entry.family == UInt8(AF_INET) {
                    if !ipv4.contains(entry.value) {
                        ipv4.append(entry.value)
                    }
                } else if entry.family == UInt8(AF_INET6) {
                    if !includeLinkLocalIPv6 && entry.isLinkLocalV6 {
                        continue
                    }
                    if !ipv6.contains(entry.value) {
                        ipv6.append(entry.value)
                    }
                }
            }
            return (ipv4, ipv6)
        }

        func primaryOnly(
            _ addresses: (ipv4: [String], ipv6: [String])
        ) -> (ipv4: [String], ipv6: [String]) {
            let ipv4 = addresses.ipv4.sorted()
            let ipv6 = addresses.ipv6.sorted()
            return (
                ipv4: ipv4.isEmpty ? [] : [ipv4[0]],
                ipv6: ipv6.isEmpty ? [] : [ipv6[0]]
            )
        }

        for pointer in sequence(first: firstAddr, next: { $0.pointee.ifa_next }) {
            let interface = pointer.pointee
            guard let addrPtr = interface.ifa_addr else {
                continue
            }
            let addrFamily = addrPtr.pointee.sa_family
            if addrFamily != UInt8(AF_INET) && addrFamily != UInt8(AF_INET6) {
                continue
            }
            let flags = Int32(interface.ifa_flags)
            if (flags & IFF_UP) == 0 || (flags & IFF_LOOPBACK) != 0 {
                continue
            }

            var addr = addrPtr.pointee
            var hostName = [CChar](repeating: 0, count: Int(NI_MAXHOST))
            let result = getnameinfo(
                &addr,
                socklen_t(addrPtr.pointee.sa_len),
                &hostName,
                socklen_t(hostName.count),
                nil,
                socklen_t(0),
                NI_NUMERICHOST
            )
            if result == 0 {
                var value = String(cString: hostName).trimmingCharacters(in: .whitespacesAndNewlines)
                if value.isEmpty {
                    continue
                }
                if addrFamily == UInt8(AF_INET6) {
                    value = value.split(separator: "%", maxSplits: 1, omittingEmptySubsequences: false)
                        .first
                        .map(String.init) ?? value
                    let lower = value.lowercased()
                    if value == "::" || value == "::1" {
                        continue
                    }
                    entries.append(
                        IPEntry(
                            interfaceName: String(cString: interface.ifa_name),
                            family: addrFamily,
                            value: value,
                            isLinkLocalV6: lower.hasPrefix("fe80:")
                        )
                    )
                } else {
                    if value == "0.0.0.0" {
                        continue
                    }
                    entries.append(
                        IPEntry(
                            interfaceName: String(cString: interface.ifa_name),
                            family: addrFamily,
                            value: value,
                            isLinkLocalV6: false
                        )
                    )
                }
            }
        }

        if !preferredInterfaceNames.isEmpty {
            let preferredEntries = entries.filter { preferredInterfaceNames.contains($0.interfaceName) }
            let preferredGlobal = collectAddresses(from: preferredEntries, includeLinkLocalIPv6: false)
            if !preferredGlobal.ipv4.isEmpty || !preferredGlobal.ipv6.isEmpty {
                return primaryOnly(preferredGlobal)
            }

            let preferredWithLinkLocal = collectAddresses(
                from: preferredEntries,
                includeLinkLocalIPv6: true
            )
            if !preferredWithLinkLocal.ipv4.isEmpty || !preferredWithLinkLocal.ipv6.isEmpty {
                return primaryOnly(preferredWithLinkLocal)
            }
        }

        let global = collectAddresses(from: entries, includeLinkLocalIPv6: false)
        if !global.ipv4.isEmpty || !global.ipv6.isEmpty {
            return primaryOnly(global)
        }

        return primaryOnly(collectAddresses(from: entries, includeLinkLocalIPv6: true))
    }
}
