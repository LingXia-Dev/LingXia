import Foundation

/// Single source of truth for the active Apple host integration.
///
/// The default shell path and the custom-controller path both ultimately route
/// runtime callbacks through this context. That avoids split-brain ownership
/// between facade-level and platform-level singletons.
@MainActor
enum LxAppActiveHost {
    private static var activeShellRef: LxAppShell?
    private static var activeControllerRef: LxAppController?

    static var activeShell: LxAppShell? { activeShellRef }
    static var activeController: LxAppController? { activeControllerRef }

    static func activate(shell: LxAppShell) {
        activeShellRef = shell
        activeControllerRef = nil
    }

    static func activate(controller: LxAppController) {
        activeControllerRef = controller
        activeShellRef = nil
    }

    static func clear(shell: LxAppShell) {
        guard activeShellRef === shell else { return }
        activeShellRef = nil
    }
}
