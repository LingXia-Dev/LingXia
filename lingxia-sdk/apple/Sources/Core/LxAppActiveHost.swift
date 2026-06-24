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
        // Keep any active controller: a custom-controller host (the runner) mounts
        // a shell as its content surface but still needs its controller to stay the
        // open router, so reopens (e.g. lxapp restart) route back through it instead
        // of falling to the standard window.
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
