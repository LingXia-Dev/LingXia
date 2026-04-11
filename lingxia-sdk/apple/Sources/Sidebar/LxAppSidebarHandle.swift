import Foundation

/// An opaque, `Sendable` handle referencing a registered `LxAppSidebarProviding`.
///
/// Handles are created via `LxAppSidebarHandle.register(_:)` and passed
/// to `.swiftNative(handle)` in the shell configuration.
public struct LxAppSidebarHandle: Sendable, Hashable {
    internal let id: UUID

    private init(id: UUID) {
        self.id = id
    }

    /// Register a sidebar provider and get a handle to reference it.
    @MainActor
    public static func register(_ provider: any LxAppSidebarProviding) -> LxAppSidebarHandle {
        let handle = LxAppSidebarHandle(id: UUID())
        LxAppSidebarRegistry.shared.providers[handle.id] = provider
        return handle
    }
}

/// Internal registry mapping handles to providers.
@MainActor
internal final class LxAppSidebarRegistry {
    static let shared = LxAppSidebarRegistry()
    var providers: [UUID: any LxAppSidebarProviding] = [:]

    func resolve(_ handle: LxAppSidebarHandle) -> (any LxAppSidebarProviding)? {
        providers[handle.id]
    }
}
