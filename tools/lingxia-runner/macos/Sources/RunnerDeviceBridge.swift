import Darwin
import Foundation

private struct RunnerHingePayload: Encodable, Sendable {
    let x: Double
    let y: Double
    let width: Double
    let height: Double
    let orientation: String
    let posture: String

    init(_ hinge: RunnerHingeSpec) {
        self.x = Double(hinge.x)
        self.y = Double(hinge.y)
        self.width = Double(hinge.width)
        self.height = Double(hinge.height)
        self.orientation = hinge.orientation.rawValue
        self.posture = hinge.posture.rawValue
    }
}

private struct RunnerDevicePostureEntryPayload: Encodable, Sendable {
    let id: String
    let label: String
    let width: Double
    let height: Double
    let hinge: RunnerHingePayload?

    init(_ posture: RunnerDevicePosture) {
        self.id = posture.id
        self.label = posture.label
        self.width = Double(posture.width)
        self.height = Double(posture.height)
        self.hinge = posture.hinge.map(RunnerHingePayload.init)
    }
}

private struct RunnerDeviceEntryPayload: Encodable, Sendable {
    let id: String
    let name: String
    let group: String
    let width: Double
    let height: Double
    let pixelRatio: Double
    let current: Bool
    let postures: [RunnerDevicePostureEntryPayload]?
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
    let posture: String?
    let hinge: RunnerHingePayload?
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
            current: device.id == current,
            postures: device.postures?.map(RunnerDevicePostureEntryPayload.init)
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
        contrast: RunnerApp.shared.simulatedHighContrast ? "high" : "standard",
        posture: RunnerApp.shared.activePosture?.id,
        hinge: effective.activeHinge.map(RunnerHingePayload.init)
    )
}

/// Split an optional `"<device-id>:<posture-id>"` or `"<device-id>@<posture-id>"`
/// selector so callers can resolve foldable presets with or without a posture suffix.
private func parseDeviceSelector(_ raw: String) -> (deviceId: String, postureId: String?) {
    if let sep = raw.firstIndex(where: { $0 == ":" || $0 == "@" }) {
        let deviceId = String(raw[..<sep])
        let postureId = String(raw[raw.index(after: sep)...])
        return (deviceId, postureId.isEmpty ? nil : postureId)
    }
    return (raw, nil)
}

@MainActor
private func setDevice(
    id: String?,
    posture: String? = nil,
    landscape: Bool?,
    appearance: RunnerAppearance?,
    textScale: CGFloat?,
    highContrast: Bool?
) -> RunnerDeviceStatePayload? {
    if let id {
        let parsed = parseDeviceSelector(id)
        guard let device = MobileDeviceSize.allCases.first(where: { $0.id == parsed.deviceId }) else {
            return nil
        }
        let requestedPostureId = posture ?? parsed.postureId
        let resolvedPosture: RunnerDevicePosture?
        if let requestedPostureId {
            guard let matched = device.posture(id: requestedPostureId) else {
                return nil
            }
            resolvedPosture = matched
        } else {
            resolvedPosture = nil
        }
        let orientation = landscape.map {
            $0 ? RunnerDeviceOrientation.landscape : .portrait
        }
        RunnerApp.shared.applyDeviceSize(
            device,
            orientation: orientation,
            posture: resolvedPosture
        )
    } else if let posture {
        guard RunnerApp.shared.setDevicePosture(id: posture) else {
            return nil
        }
        if let landscape {
            RunnerApp.shared.setDeviceOrientation(landscape ? .landscape : .portrait)
        }
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

@_cdecl("lingxia_runner_device_set_posture_json")
func lingxiaRunnerDeviceSetPostureJSON(
    _ posture: UnsafePointer<CChar>?
) -> UnsafeMutablePointer<CChar>? {
    guard let posture else { return nil }
    let postureId = String(cString: posture)
    guard let state = onRunnerMain({
        setDevice(
            id: nil,
            posture: postureId,
            landscape: nil,
            appearance: nil,
            textScale: nil,
            highContrast: nil
        )
    }) else {
        return nil
    }
    return encodedCString(state)
}
