import Darwin
import Foundation

private struct RunnerDeviceEntryPayload: Encodable, Sendable {
    let id: String
    let name: String
    let group: String
    let width: Int
    let height: Int
    let current: Bool
}

private struct RunnerDeviceStatePayload: Encodable, Sendable {
    let id: String
    let name: String
    let group: String
    let width: Int
    let height: Int
    let landscape: Bool
    let appearance: String
    let capsule: Bool
}

@MainActor
private func deviceEntries() -> [RunnerDeviceEntryPayload] {
    let current = RunnerApp.shared.selectedDeviceSize.id
    return MobileDeviceSize.allCases.map { device in
        RunnerDeviceEntryPayload(
            id: device.id,
            name: device.name,
            group: device.group,
            width: Int(device.width),
            height: Int(device.height),
            current: device.id == current
        )
    }
}

@MainActor
private func deviceState() -> RunnerDeviceStatePayload {
    let selected = RunnerApp.shared.selectedDeviceSize
    let effective = RunnerApp.shared.deviceSize
    return RunnerDeviceStatePayload(
        id: selected.id,
        name: selected.name,
        group: selected.group,
        width: Int(effective.width),
        height: Int(effective.height),
        landscape: RunnerApp.shared.deviceOrientation == .landscape,
        appearance: RunnerApp.shared.simulatedAppearance.rawValue,
        capsule: RunnerApp.shared.capsuleEnabled
    )
}

@MainActor
private func setDevice(
    id: String?,
    landscape: Bool?,
    appearance: RunnerAppearance?,
    capsule: Bool?
) -> RunnerDeviceStatePayload? {
    if let id {
        guard let device = MobileDeviceSize.allCases.first(where: { $0.id == id }) else {
            return nil
        }
        let orientation = landscape.map {
            $0 ? RunnerDeviceOrientation.landscape : .portrait
        }
        RunnerApp.shared.setDeviceSize(device, orientation: orientation)
    } else if let landscape {
        RunnerApp.shared.setDeviceOrientation(landscape ? .landscape : .portrait)
    }
    if let appearance {
        RunnerApp.shared.setAppearance(appearance)
    }
    if let capsule {
        RunnerApp.shared.setCapsuleEnabled(capsule)
    }
    return deviceState()
}

private func onRunnerMain<T: Sendable>(
    _ operation: @escaping @MainActor @Sendable () -> T
) -> T {
    if Thread.isMainThread {
        return MainActor.assumeIsolated { operation() }
    }
    return DispatchQueue.main.sync {
        MainActor.assumeIsolated { operation() }
    }
}

private func encodedCString<T: Encodable>(_ value: T) -> UnsafeMutablePointer<CChar>? {
    guard let data = try? JSONEncoder().encode(value),
          let json = String(data: data, encoding: .utf8)
    else {
        return nil
    }
    return strdup(json)
}

@_cdecl("lingxia_runner_device_list_json")
func lingxiaRunnerDeviceListJSON() -> UnsafeMutablePointer<CChar>? {
    encodedCString(onRunnerMain { deviceEntries() })
}

@_cdecl("lingxia_runner_device_get_json")
func lingxiaRunnerDeviceGetJSON() -> UnsafeMutablePointer<CChar>? {
    encodedCString(onRunnerMain { deviceState() })
}

@_cdecl("lingxia_runner_device_set_json")
func lingxiaRunnerDeviceSetJSON(
    _ id: UnsafePointer<CChar>?,
    _ landscape: Int32,
    _ appearance: Int32,
    _ capsule: Int32
) -> UnsafeMutablePointer<CChar>? {
    let deviceId = id.map { String(cString: $0) }
    let requestedOrientation: Bool? = switch landscape {
    case 0: false
    case 1: true
    default: nil
    }
    let requestedAppearance: RunnerAppearance? = switch appearance {
    case 0: .system
    case 1: .light
    case 2: .dark
    default: nil
    }
    let requestedCapsule: Bool? = switch capsule {
    case 0: false
    case 1: true
    default: nil
    }
    guard let state = onRunnerMain({
        setDevice(
            id: deviceId,
            landscape: requestedOrientation,
            appearance: requestedAppearance,
            capsule: requestedCapsule
        )
    }) else {
        return nil
    }
    return encodedCString(state)
}
