import Foundation
#if os(iOS)

/// Window overlays; iOS has none yet, so the feedback layer is not installed there.
public enum GpuOverlays {
    static func open(token: Int64) -> Bool { false }
    static func input(token: Int64, block: Bool) {}
    static func focus(token: Int64, focused: Bool) {}
    static func close(token: Int64) {}
}
#endif
