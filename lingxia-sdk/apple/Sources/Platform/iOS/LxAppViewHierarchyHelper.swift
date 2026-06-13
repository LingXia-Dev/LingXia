#if os(iOS)
import UIKit

/// iOS-specific view hierarchy helper
@MainActor
class LxAppViewHierarchyHelper {
    /// Finds the topmost view controller in the hierarchy
    static func findTopmostViewController(from viewController: UIViewController) -> UIViewController {
        if let presentedVC = viewController.presentedViewController {
            return findTopmostViewController(from: presentedVC)
        }

        if let navController = viewController as? UINavigationController,
           let topVC = navController.topViewController {
            return findTopmostViewController(from: topVC)
        }

        if let tabController = viewController as? UITabBarController,
           let selectedVC = tabController.selectedViewController {
            return findTopmostViewController(from: selectedVC)
        }

        return viewController
    }

    /// Find specific view controller type in hierarchy
    static func findSpecificViewController<T>(in viewController: UIViewController?) -> T? {
        guard let viewController = viewController else { return nil }

        if let targetVC = viewController as? T {
            return targetVC
        }

        if let navController = viewController as? UINavigationController {
            return findSpecificViewController(in: navController.topViewController)
        }

        if let presentedVC = viewController.presentedViewController {
            return findSpecificViewController(in: presentedVC)
        }

        return nil
    }
}
#endif
