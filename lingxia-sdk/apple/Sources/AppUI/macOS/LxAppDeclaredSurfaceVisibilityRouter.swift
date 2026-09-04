#if os(macOS)
import Foundation

@MainActor
protocol LxAppDeclaredSurfaceVisibilityRouting: AnyObject {
    func openManagedSurface(id: String, role: String?, edge: String?) -> Bool
    func closeManagedSurface(id: String) -> Bool
}

extension LxAppMacAppUIRuntime: LxAppDeclaredSurfaceVisibilityRouting {}

/// Strictly routes the generic surface API to declared/runtime-managed
/// surfaces. A declaration miss is an error, never a built-in-page fallback.
@MainActor
enum LxAppDeclaredSurfaceVisibilityRouter {
    static func setVisible(
        in runtime: LxAppDeclaredSurfaceVisibilityRouting,
        id: String,
        visible: Bool,
        role: String?,
        edge: String?
    ) -> Bool {
        if visible {
            return runtime.openManagedSurface(id: id, role: role, edge: edge)
        }
        return runtime.closeManagedSurface(id: id)
    }
}
#endif
