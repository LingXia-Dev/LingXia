import Foundation
import OSLog

#if os(iOS)
import UIKit

/// Adaptive Surface Layout — iOS full-screen aside reconciler.
///
/// The shared Rust core owns the surface graph and, after every mutation,
/// re-derives a `LayoutPresentationPlan` and hands it here. A phone has no
/// side-by-side dock: an aside presents full-screen, the same way the primary
/// lxapp page is shown. The plan keeps desired asides in `plan.asides` even
/// under compact full-screen presentation so this reconciler can dismiss
/// surfaces removed from the graph without treating `windowId` as an app id.
///
/// The contract mirrors the macOS reconciler's desired-set vs presented-set:
///
///   * an aside the plan lists is already on-screen — `LxAppSurface.present`
///     shows a full-screen surface eagerly when it is presented, so a desired
///     aside is left untouched (idempotent, no flicker);
///   * an aside the reconciler previously presented that is no longer in the
///     plan is dismissed (the core dropped it).
///
/// The active main is driven by the host navigation path (open/navigate/close),
/// not by this reconciler, so it is not re-presented here. Floats keep their
/// positioned-popup presentation and their own backdrop-tap / close lifecycle,
/// so they are left to the existing surface path.
@MainActor
enum LxAppLayoutReconcileriOS {
    private static let log = OSLog(subsystem: "LingXia", category: "LayoutReconciler")

    private struct LayoutPresentationPlan: Decodable {
        let sizeClass: String
        let bottomOwner: String
        let switcherForm: String
        let splitForm: String
        let mains: [String]
        let activeMainId: String?
        let asides: [PlanAside]
        let floats: [PlanFloat]
        let tree: LxAppJSONValue?
    }

    private struct PlanAside: Decodable {
        let id: String
        let edge: String?
        let preferredSize: Double?
    }

    private struct PlanFloat: Decodable {
        let id: String
    }

    static func reconcile(windowId: String, json: String) -> Bool {
        guard let data = json.data(using: .utf8),
              let plan = try? JSONDecoder().decode(LayoutPresentationPlan.self, from: data) else {
            LXLog.error("presentLayout: failed to parse layout json window=\(windowId)", category: "LayoutReconciler")
            return false
        }

        // On iOS today every desired main/aside is a full-screen surface. The
        // same plan.asides list also lets future iPad split skins keep desired
        // asides alive without changing the bridge contract.
        let desiredIds = Set(plan.asides.map { $0.id }).union(plan.mains)

        for id in LxAppSurface.presentedFullScreenIds().subtracting(desiredIds) {
            LxAppSurface.dismissFullScreen(id: id)
        }

        return true
    }
}
#endif
