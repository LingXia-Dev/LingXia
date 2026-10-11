#if os(iOS)
import UIKit

/// Overlay mounts override WebKit; composited mounts must first win WebKit's
/// own hit test, so HTML painted above them retains its touches.
@MainActor
final class NativeComponentHitRouter {
    static let shared = NativeComponentHitRouter()

    private struct Entry {
        weak var view: UIView?
        weak var container: UIScrollView?
        let requiresWebKitHit: Bool
    }
    private var entries: [ObjectIdentifier: Entry] = [:]

    func registerNativeView(_ view: UIView, in container: UIScrollView, requiresWebKitHit: Bool = false) {
        entries[ObjectIdentifier(view)] = Entry(view: view, container: container, requiresWebKitHit: requiresWebKitHit)
    }

    func unregisterNativeView(_ view: UIView) {
        entries.removeValue(forKey: ObjectIdentifier(view))
    }

    func nativeView(at point: CGPoint, in hostView: UIView, webHit: UIView?, event: UIEvent?) -> UIView? {
        entries = entries.filter { $0.value.view != nil && $0.value.container != nil }
        let candidates = entries.values.compactMap { entry -> UIView? in
            guard let view = entry.view, let container = entry.container,
                  view.window != nil, view.isDescendant(of: hostView),
                  Self.accepts(point, in: hostView, for: view, event: event)
            else { return nil }
            if entry.requiresWebKitHit {
                guard view.isDescendant(of: container), let webHit,
                      webHit === container || webHit.isDescendant(of: container)
                else { return nil }
            }
            return view
        }.sorted { Self.isAbove($0, $1, within: hostView) }
        for view in candidates {
            let local = hostView.convert(point, to: view)
            // Respect a container's transparent holes and disabled children.
            // Returning `view` after a nil hit would create an invisible blocker.
            if let hit = view.hitTest(local, with: event) { return hit }
        }
        return nil
    }

    private static func accepts(_ point: CGPoint, in host: UIView, for view: UIView, event: UIEvent?) -> Bool {
        var current: UIView? = view
        var alpha: CGFloat = 1
        while let node = current {
            alpha *= node.alpha
            if node.isHidden || !node.isUserInteractionEnabled || alpha <= 0.01 { return false }
            let local = host.convert(point, to: node)
            if node.clipsToBounds && !node.bounds.contains(local) { return false }
            if let mask = node.layer.mask as? CAShapeLayer, let path = mask.path,
               !path.contains(local, using: mask.fillRule == .evenOdd ? .evenOdd : .winding) { return false }
            if node === host { return true }
            current = node.superview
        }
        return false
    }

    /// Dictionary insertion order is unrelated to the view's paint order.
    private static func isAbove(_ lhs: UIView, _ rhs: UIView, within host: UIView) -> Bool {
        func chain(_ view: UIView) -> [UIView] {
            var result: [UIView] = []
            var node: UIView? = view
            while let current = node {
                result.append(current)
                if current === host { break }
                node = current.superview
            }
            return result.reversed()
        }
        let left = chain(lhs), right = chain(rhs)
        for (a, b) in zip(left, right) where a !== b {
            if a.layer.zPosition != b.layer.zPosition { return a.layer.zPosition > b.layer.zPosition }
            guard let siblings = a.superview?.subviews,
                  let ai = siblings.firstIndex(of: a), let bi = siblings.firstIndex(of: b) else { return false }
            return ai > bi
        }
        return left.count > right.count
    }
}
#endif
