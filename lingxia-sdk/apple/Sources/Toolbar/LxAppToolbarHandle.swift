import Foundation

/// An opaque, `Sendable` handle referencing a registered `LxAppToolbarProviding`.
public struct LxAppToolbarHandle: Sendable, Hashable {
    internal let id: UUID

    private init(id: UUID) {
        self.id = id
    }

    /// Register a toolbar provider and get a handle to reference it.
    @MainActor
    public static func register(_ provider: any LxAppToolbarProviding) -> LxAppToolbarHandle {
        let handle = LxAppToolbarHandle(id: UUID())
        LxAppToolbarRegistry.shared.providers[handle.id] = provider
        return handle
    }
}

/// Internal registry mapping handles to providers.
@MainActor
internal final class LxAppToolbarRegistry {
    static let shared = LxAppToolbarRegistry()
    var providers: [UUID: any LxAppToolbarProviding] = [:]

    func resolve(_ handle: LxAppToolbarHandle) -> (any LxAppToolbarProviding)? {
        providers[handle.id]
    }
}
