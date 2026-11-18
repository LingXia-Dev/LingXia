import Darwin
import Foundation

private struct RunnerDeviceEntryPayload: Encodable, Sendable {
    let id: String
    let name: String
    let group: String
    let width: Double
    let height: Double
    let pixelRatio: Double
    let current: Bool
}

private struct RunnerDeviceStatePayload: Encodable, Sendable {
    let id: String
    let name: String
    let group: String
    let width: Double
    let height: Double
    let pixelRatio: Double
    let landscape: Bool
    let appearance: String
    let textScale: Double
    let contrast: String
}

@MainActor
private func deviceEntries() -> [RunnerDeviceEntryPayload] {
    let current = RunnerApp.shared.selectedDeviceSize.id
    return MobileDeviceSize.allCases.map { device in
        RunnerDeviceEntryPayload(
            id: device.id,
            name: device.name,
            group: device.group,
            width: Double(device.width),
            height: Double(device.height),
            pixelRatio: Double(device.pixelRatio),
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
        width: Double(effective.width),
        height: Double(effective.height),
        pixelRatio: Double(selected.pixelRatio),
        landscape: RunnerApp.shared.deviceOrientation == .landscape,
        appearance: RunnerApp.shared.simulatedAppearance.rawValue,
        textScale: Double(RunnerApp.shared.simulatedTextScale),
        contrast: RunnerApp.shared.simulatedHighContrast ? "high" : "standard"
    )
}

@MainActor
private func setDevice(
    id: String?,
    landscape: Bool?,
    appearance: RunnerAppearance?,
    textScale: CGFloat?,
    highContrast: Bool?
) -> RunnerDeviceStatePayload? {
    if let id {
        guard let device = MobileDeviceSize.allCases.first(where: { $0.id == id }) else {
            return nil
        }
        let orientation = landscape.map {
            $0 ? RunnerDeviceOrientation.landscape : .portrait
        }
        RunnerApp.shared.applyDeviceSize(device, orientation: orientation)
    } else if let landscape {
        RunnerApp.shared.setDeviceOrientation(landscape ? .landscape : .portrait)
    }
    if let appearance {
        RunnerApp.shared.setAppearance(appearance)
    }
    if textScale != nil || highContrast != nil {
        RunnerApp.shared.setAccessibility(textScale: textScale, highContrast: highContrast)
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
    _ textScale: Double,
    _ contrast: Int32
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
    let requestedTextScale: CGFloat? = textScale > 0 ? CGFloat(textScale) : nil
    let requestedContrast: Bool? = switch contrast {
    case 0: false
    case 1: true
    default: nil
    }
    guard let state = onRunnerMain({
        setDevice(
            id: deviceId,
            landscape: requestedOrientation,
            appearance: requestedAppearance,
            textScale: requestedTextScale,
            highContrast: requestedContrast
        )
    }) else {
        return nil
    }
    return encodedCString(state)
}
