import Foundation
import OSLog

#if os(iOS)
import UIKit

/// Adaptive Surface Layout — iOS aside reconciler.
///
/// The shared Rust core owns the surface graph and, after every mutation,
/// re-derives a `LayoutPresentationPlan` and hands it here. The plan's `panel`
/// slot drives `LxAppAsideRegion`, which docks its children beside the main
/// on a regular-width window and covers the main otherwise. Other asides still
/// present full-screen, the same way the primary lxapp page is shown; the plan
/// keeps them in `plan.asides` so this reconciler can dismiss surfaces removed
/// from the graph without treating `windowId` as an app id.
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

    /// The last committed plan sequence applied per window.
    private static var lastSequenceByWindow: [String: UInt64] = [:]

    /// Notification posted on every reconciled adaptive layout plan so iOS /
    /// iPadOS / iPhone Duo shells switch between `compact` (`< 600 pt`) and
    /// `regular` (`>= 600 pt` docked sidebar + multi-tab aside) purely by width.
    static let layoutPlanDidUpdateNotification = Notification.Name("LxAppLayoutReconcileriOSPlanDidUpdate")

    struct PlanHingeRect: Decodable {
        let x: Double
        let y: Double
        let width: Double
        let height: Double
        let orientation: String
        let posture: String
    }

    struct PlanAsideSlot: Decodable {
        let kind: String
        let edge: String?
        let children: [String]
        let activeChild: String?
        let visible: Bool
        let overlay: Bool
        let collapsed: Bool
        let maximized: Bool
    }

    struct PlanUnifiedAsideSlot: Decodable {
        let tabs: [String]
        let activeTab: String?
        let visible: Bool
        let overlay: Bool
        let collapsed: Bool
        let maximized: Bool
    }

    struct LayoutPresentationPlan: Decodable {
        let sequence: UInt64?
        let sizeClass: String
        let bottomOwner: String
        let switcherForm: String
        let splitForm: String
        let mains: [String]
        let activeMainId: String?
        let asides: [PlanAside]
        let asideSlots: [PlanAsideSlot]?
        let unifiedAsideSlot: PlanUnifiedAsideSlot?
        let floats: [PlanFloat]
        let tree: LxAppJSONValue?
        let hinge: PlanHingeRect?
    }

    struct PlanAside: Decodable {
        let id: String
        let edge: String?
        let preferredSize: Double?
    }

    struct PlanFloat: Decodable {
        let id: String
    }

    private(set) static var latestPlanByWindow: [String: LayoutPresentationPlan] = [:]

    static func reconcile(windowId: String, json: String) -> Bool {
        guard let data = json.data(using: .utf8),
              let plan = try? JSONDecoder().decode(LayoutPresentationPlan.self, from: data) else {
            LXLog.error("presentLayout: failed to parse layout json window=\(windowId)", category: "LayoutReconciler")
            return false
        }
        // Plans hop to the main queue: drop one that arrives after a newer
        // commit, as the macOS reconciler does.
        if let sequence = plan.sequence, sequence > 0 {
            let last = lastSequenceByWindow[windowId] ?? 0
            if sequence < last { return true }
            lastSequenceByWindow[windowId] = sequence
        }

        latestPlanByWindow[windowId] = plan
        let desiredIds = Set(plan.asides.map { $0.id }).union(plan.mains)

        for id in LxAppSurface.presentedFullScreenIds().subtracting(desiredIds) {
            LxAppSurface.dismissFullScreen(id: id)
        }
        // The side panel follows the plan's `panel` slot; it closes the children the plan dropped.
        LxAppAsideRegion.current?.apply(plan: plan.asideSlots)

        NotificationCenter.default.post(
            name: layoutPlanDidUpdateNotification,
            object: nil,
            userInfo: ["windowId": windowId, "sizeClass": plan.sizeClass, "switcherForm": plan.switcherForm]
        )

        return true
    }
}
#endif
