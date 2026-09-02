import CLingXiaRustAPI
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
        // The home lxapp declares its sidebar actions once, at launch, and the
        // declaration is pushed at that moment. A shell that appears later --
        // the Runner switching to a desktop shape, a window opened after boot
        // -- has to ask for it, or it shows an empty sidebar until the lxapp's
        // next launch. The shell holds what comes back and projects it once its
        // sidebar exists. The Windows shell runtime replays the same way when
        // it installs.
        _ = shellReapplyChrome()
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
