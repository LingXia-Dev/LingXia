import Foundation
import QuartzCore
import WebKit
import CLingXiaRustAPI
#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

struct GpuWebViewClipSpec: Decodable {
    let bounds: [CGFloat]
    let radius: CGFloat
    let radii: [[CGFloat]]?
}

struct GpuWebViewMountSpec: Decodable {
    let webId: String
    let bounds: [CGFloat]
    let visibleBounds: [CGFloat]
    let radius: CGFloat
    let radii: [[CGFloat]]?
    let clips: [GpuWebViewClipSpec]
    /// Later web views' holes over this one: masked out, since native views do not stack in
    /// paint order.
    let occluders: [GpuWebViewClipSpec]?
    let degraded: Bool
    let accessibilityHidden: Bool
    /// The page handles its own drags: one starting on it never passes to the View page.
    let handlesDrags: Bool?
    /// The View page's appearance, which the web page's `prefers-color-scheme` follows.
    let dark: Bool?
}

#if os(iOS)
private func gpuPicturePng(_ image: UIImage) -> Data? { image.pngData() }
#elseif os(macOS)
private func gpuPicturePng(_ image: NSImage) -> Data? {
    guard let cg = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else { return nil }
    return NSBitmapImageRep(cgImage: cg).representation(using: .png, properties: [:])
}
#endif

private func gpuRoundedPath(
    bounds: [CGFloat],
    radius: CGFloat,
    radii: [[CGFloat]]?,
    offset: CGPoint
) -> CGPath? {
    guard bounds.count == 4 else { return nil }
    let x = bounds[0] - offset.x
    let y = bounds[1] - offset.y
    let w = max(0, bounds[2])
    let h = max(0, bounds[3])
    guard w > 0, h > 0 else { return nil }

    var corners: [(CGFloat, CGFloat)]
    if let r = radii, r.count == 4 {
        corners = r.map { pair in
            pair.count == 2 ? (max(0, pair[0]), max(0, pair[1])) : (0, 0)
        }
    } else if radius > 0 {
        corners = Array(repeating: (radius, radius), count: 4)
    } else {
        return nil
    }

    let hasAnyRadius = corners.contains { $0.0 > 0.01 || $0.1 > 0.01 }
    if !hasAnyRadius {
        return nil
    }

    // CSS corner-radius proportional scale down when adjacent radii exceed box edges.
    let topSum = corners[0].0 + corners[1].0
    let bottomSum = corners[3].0 + corners[2].0
    let leftSum = corners[0].1 + corners[3].1
    let rightSum = corners[1].1 + corners[2].1
    var f: CGFloat = 1.0
    if topSum > w, topSum > 0 { f = min(f, w / topSum) }
    if bottomSum > w, bottomSum > 0 { f = min(f, w / bottomSum) }
    if leftSum > h, leftSum > 0 { f = min(f, h / leftSum) }
    if rightSum > h, rightSum > 0 { f = min(f, h / rightSum) }
    if f < 1.0 {
        corners = corners.map { ($0.0 * f, $0.1 * f) }
    }

    let k: CGFloat = 0.5522847498307936
    let (tlx, tly) = corners[0]
    let (trx, try_) = corners[1]
    let (brx, bry) = corners[2]
    let (blx, bly) = corners[3]

    let path = CGMutablePath()
    path.move(to: CGPoint(x: x + tlx, y: y))
    path.addLine(to: CGPoint(x: x + w - trx, y: y))
    if trx > 0 || try_ > 0 {
        path.addCurve(
            to: CGPoint(x: x + w, y: y + try_),
            control1: CGPoint(x: x + w - trx * (1 - k), y: y),
            control2: CGPoint(x: x + w, y: y + try_ * (1 - k))
        )
    }
    path.addLine(to: CGPoint(x: x + w, y: y + h - bry))
    if brx > 0 || bry > 0 {
        path.addCurve(
            to: CGPoint(x: x + w - brx, y: y + h),
            control1: CGPoint(x: x + w, y: y + h - bry * (1 - k)),
            control2: CGPoint(x: x + w - brx * (1 - k), y: y + h)
        )
    }
    path.addLine(to: CGPoint(x: x + blx, y: y + h))
    if blx > 0 || bly > 0 {
        path.addCurve(
            to: CGPoint(x: x, y: y + h - bly),
            control1: CGPoint(x: x + blx * (1 - k), y: y + h),
            control2: CGPoint(x: x, y: y + h - bly * (1 - k))
        )
    }
    path.addLine(to: CGPoint(x: x, y: y + tly))
    if tlx > 0 || tly > 0 {
        path.addCurve(
            to: CGPoint(x: x + tlx, y: y),
            control1: CGPoint(x: x, y: y + tly * (1 - k)),
            control2: CGPoint(x: x + tlx * (1 - k), y: y)
        )
    }
    path.closeSubpath()
    return path
}

private func buildMaskChain(spec: GpuWebViewMountSpec, clipOrigin: CGPoint) -> CALayer? {
    var paths: [CGPath] = []
    if let own = gpuRoundedPath(bounds: spec.bounds, radius: spec.radius, radii: spec.radii, offset: clipOrigin) {
        paths.append(own)
    }
    for clip in spec.clips {
        if let anc = gpuRoundedPath(bounds: clip.bounds, radius: clip.radius, radii: clip.radii, offset: clipOrigin) {
            paths.append(anc)
        }
    }
    let occluders = spec.occluders ?? []
    if !occluders.isEmpty, spec.visibleBounds.count == 4 {
        // Even-odd: the slot's whole box, less each occluder.
        let cut = CGMutablePath()
        cut.addRect(CGRect(x: spec.visibleBounds[0] - clipOrigin.x, y: spec.visibleBounds[1] - clipOrigin.y,
                           width: spec.visibleBounds[2], height: spec.visibleBounds[3]))
        for occluder in occluders where occluder.bounds.count == 4 {
            if let rounded = gpuRoundedPath(bounds: occluder.bounds, radius: occluder.radius,
                                            radii: occluder.radii, offset: clipOrigin) {
                cut.addPath(rounded)
            } else {
                cut.addRect(CGRect(x: occluder.bounds[0] - clipOrigin.x, y: occluder.bounds[1] - clipOrigin.y,
                                   width: occluder.bounds[2], height: occluder.bounds[3]))
            }
        }
        paths.append(cut)
    }
    guard let first = paths.first else { return nil }
    let root = CAShapeLayer()
    root.actions = ["path": NSNull(), "frame": NSNull(), "position": NSNull(), "bounds": NSNull()]
    root.path = first
    root.fillRule = paths.count == 1 && !occluders.isEmpty ? .evenOdd : .nonZero
    var current = root
    for (index, nextPath) in paths.enumerated().dropFirst() {
        let sub = CAShapeLayer()
        sub.actions = ["path": NSNull(), "frame": NSNull(), "position": NSNull(), "bounds": NSNull()]
        sub.path = nextPath
        sub.fillRule = index == paths.count - 1 && !occluders.isEmpty ? .evenOdd : .nonZero
        current.mask = sub
        current = sub
    }
    return root
}

#if os(iOS)
@MainActor
final class GpuMetalSurfaceView: UIView {
    override class var layerClass: AnyClass { CAMetalLayer.self }

    init() {
        super.init(frame: .zero)
        isOpaque = true
        layer.isOpaque = true
        isUserInteractionEnabled = false
        layer.contentsGravity = .topLeft
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is unavailable") }
}

@MainActor
final class GpuWebViewSlot: UIView {
    let webId: String
    weak var webView: WKWebView?
    var accessibilitySuppressed = false
    var handlesDrags = false
    /// What the last sync asked for; a snapshot shows a hidden slot only while it runs.
    var wantsHidden = false
    /// The appearance last applied, and to which web view.
    var appliedDark: (Bool, ObjectIdentifier)?

    init(webId: String) {
        self.webId = webId
        super.init(frame: .zero)
        clipsToBounds = true
        backgroundColor = .clear
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is unavailable") }

    func attachIfNeeded(token: Int64) {
        if let existing = webView, existing.superview === self { return }
        let raw = gpuPageEmbeddedWebView(token, webId)
        guard raw != 0, let ptr = UnsafeRawPointer(bitPattern: raw) else { return }
        let wv = Unmanaged<WKWebView>.fromOpaque(ptr).takeUnretainedValue()
        wv.translatesAutoresizingMaskIntoConstraints = true
        wv.scrollView.contentInsetAdjustmentBehavior = .never
        if wv.superview !== self {
            wv.removeFromSuperview()
            addSubview(wv)
        }
        webView = wv
    }
}

@MainActor
final class GpuWebViewContainer: UIView {
    private let token: Int64
    private var slots: [String: GpuWebViewSlot] = [:]
    private var orderedIds: [String] = []

    init(token: Int64) {
        self.token = token
        super.init(frame: .zero)
        isHidden = true
        clipsToBounds = true
        backgroundColor = .clear
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is unavailable") }

    var hasActiveMounts: Bool { !slots.isEmpty }

    var accessibleWebViews: [Any] {
        orderedIds.compactMap { id in
            guard let slot = slots[id], !slot.isHidden, !slot.accessibilitySuppressed, let wv = slot.webView else {
                return nil
            }
            return wv
        }
    }

    func webView(for webId: String) -> WKWebView? {
        guard let slot = slots[webId], !slot.isHidden else { return nil }
        return slot.webView
    }

    /// Whether a drag starting on `webId` stays with its page whatever that page can scroll.
    func handlesDrags(_ webId: String) -> Bool {
        slots[webId]?.handlesDrags ?? false
    }

    /// Pictures each of `ids` at `scale` device pixels per point for the page's core, which
    /// draws it while the live view cannot show. A hidden slot is shown for the snapshot: no
    /// hole is punched for it, so the GPU layer still covers it.
    func picture(_ ids: [String], scale: CGFloat, backing: CGFloat) {
        let token = self.token
        for id in ids {
            guard let slot = slots[id], let webView = slot.webView,
                  webView.bounds.width > 0, webView.bounds.height > 0
            else { continue }
            let hidden = slot.isHidden
            slot.isHidden = false
            let config = WKSnapshotConfiguration()
            config.rect = webView.bounds
            config.snapshotWidth = NSNumber(value: Double(webView.bounds.width * scale / max(backing, 1)))
            webView.takeSnapshot(with: config) { [weak self, weak slot] image, _ in
                MainActor.assumeIsolated {
                    if hidden, let slot, self?.slots[id] === slot, slot.wantsHidden {
                        slot.isHidden = true
                    }
                    guard let png = image.flatMap(gpuPicturePng) else { return }
                    png.withUnsafeBytes { raw in
                        gpuPageWebViewPicture(token, id, raw.bindMemory(to: UInt8.self))
                    }
                }
            }
        }
    }

    func clearAll() {
        for slot in slots.values {
            slot.webView?.removeFromSuperview()
            slot.removeFromSuperview()
        }
        slots.removeAll()
        orderedIds.removeAll()
        isHidden = true
    }

    func sync(json: String) -> Bool {
        guard let data = json.data(using: .utf8),
              let specs = try? JSONDecoder().decode([GpuWebViewMountSpec].self, from: data),
              !specs.isEmpty
        else {
            clearAll()
            return false
        }
        if isHidden {
            isHidden = false
        }

        CATransaction.begin()
        CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }

        let nextIds = specs.map(\.webId)
        for (id, slot) in slots where !nextIds.contains(id) {
            slot.webView?.removeFromSuperview()
            slot.removeFromSuperview()
            slots.removeValue(forKey: id)
        }
        orderedIds = nextIds

        for (idx, spec) in specs.enumerated() {
            let slot = slots[spec.webId] ?? {
                let created = GpuWebViewSlot(webId: spec.webId)
                slots[spec.webId] = created
                addSubview(created)
                return created
            }()
            if subviews.firstIndex(of: slot) != idx {
                insertSubview(slot, at: min(idx, subviews.count))
            }
            slot.attachIfNeeded(token: token)
            if let dark = spec.dark, let webView = slot.webView,
               slot.appliedDark.map({ $0 != (dark, ObjectIdentifier(webView)) }) ?? true {
                slot.appliedDark = (dark, ObjectIdentifier(webView))
                LxAppAppearanceRegistry.apply(dark, to: webView)
            }

            let bx = spec.bounds.count == 4 ? spec.bounds[0] : 0
            let by = spec.bounds.count == 4 ? spec.bounds[1] : 0
            let bw = spec.bounds.count == 4 ? max(1, spec.bounds[2]) : 1
            let bh = spec.bounds.count == 4 ? max(1, spec.bounds[3]) : 1

            let vx = spec.visibleBounds.count == 4 ? spec.visibleBounds[0] : bx
            let vy = spec.visibleBounds.count == 4 ? spec.visibleBounds[1] : by
            let vw = spec.visibleBounds.count == 4 ? max(0, spec.visibleBounds[2]) : 0
            let vh = spec.visibleBounds.count == 4 ? max(0, spec.visibleBounds[3]) : 0

            let hidden = spec.degraded || vw <= 0.01 || vh <= 0.01
            slot.wantsHidden = hidden
            slot.isHidden = hidden
            slot.handlesDrags = spec.handlesDrags ?? false
            slot.accessibilitySuppressed = spec.accessibilityHidden || spec.degraded
            slot.accessibilityElementsHidden = slot.accessibilitySuppressed

            if !hidden {
                let clipRect = CGRect(x: vx, y: vy, width: vw, height: vh)
                if slot.frame != clipRect {
                    slot.frame = clipRect
                }
                let innerFrame = CGRect(x: bx - clipRect.minX, y: by - clipRect.minY, width: bw, height: bh)
                if let wv = slot.webView {
                    if wv.frame != innerFrame {
                        wv.frame = innerFrame
                    }
                    if wv.isHidden {
                        wv.isHidden = false
                    }
                }
                slot.layer.mask = buildMaskChain(spec: spec, clipOrigin: clipRect.origin)
            }
        }
        return !slots.isEmpty
    }
}

#elseif os(macOS)
@MainActor
final class GpuMetalSurfaceView: NSView {
    /// The view hosts its layer, whose delegate answers no window: wgpu then never refuses a
    /// drawable over the window's occlusion state, which AppKit reports for a window on screen
    /// while its app is not active. The page decides when to present.
    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        let metal = CAMetalLayer()
        metal.isOpaque = true
        metal.maximumDrawableCount = 3
        metal.contentsGravity = .topLeft
        metal.delegate = GpuSurfaceLayerDelegate.shared
        layer = metal
        wantsLayer = true
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is unavailable") }

    override var isFlipped: Bool { true }

    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

/// A hosted layer's delegate: no implicit animations, and no window for wgpu to read.
private final class GpuSurfaceLayerDelegate: NSObject, CALayerDelegate, @unchecked Sendable {
    static let shared = GpuSurfaceLayerDelegate()
    @objc var window: AnyObject? { nil }
    func action(for layer: CALayer, forKey event: String) -> CAAction? { NSNull() }
}

@MainActor
final class GpuWebViewSlot: NSView {
    let webId: String
    weak var webView: WKWebView?
    var accessibilitySuppressed = false
    /// What the last sync asked for; a snapshot shows a hidden slot only while it runs.
    var wantsHidden = false
    /// The appearance last applied, and to which web view.
    var appliedDark: (Bool, ObjectIdentifier)?

    init(webId: String) {
        self.webId = webId
        super.init(frame: .zero)
        wantsLayer = true
        layer?.masksToBounds = true
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is unavailable") }

    override var isFlipped: Bool { true }

    func attachIfNeeded(token: Int64) {
        if let existing = webView, existing.superview === self { return }
        let raw = gpuPageEmbeddedWebView(token, webId)
        guard raw != 0, let ptr = UnsafeRawPointer(bitPattern: raw) else { return }
        let wv = Unmanaged<WKWebView>.fromOpaque(ptr).takeUnretainedValue()
        wv.translatesAutoresizingMaskIntoConstraints = true
        if wv.superview !== self {
            wv.removeFromSuperview()
            addSubview(wv)
        }
        webView = wv
    }
}

@MainActor
final class GpuWebViewContainer: NSView {
    private let token: Int64
    private var slots: [String: GpuWebViewSlot] = [:]
    private var orderedIds: [String] = []

    init(token: Int64) {
        self.token = token
        super.init(frame: .zero)
        isHidden = true
        wantsLayer = true
        layer?.masksToBounds = true
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is unavailable") }

    override var isFlipped: Bool { true }

    var hasActiveMounts: Bool { !slots.isEmpty }

    var accessibleWebViews: [Any] {
        orderedIds.compactMap { id in
            guard let slot = slots[id], !slot.isHidden, !slot.accessibilitySuppressed, let wv = slot.webView else {
                return nil
            }
            return wv
        }
    }

    func webView(for webId: String) -> WKWebView? {
        guard let slot = slots[webId], !slot.isHidden else { return nil }
        return slot.webView
    }

    /// Pictures each of `ids` at `scale` device pixels per point for the page's core, which
    /// draws it while the live view cannot show. A hidden slot is shown for the snapshot: no
    /// hole is punched for it, so the GPU layer still covers it.
    func picture(_ ids: [String], scale: CGFloat, backing: CGFloat) {
        let token = self.token
        for id in ids {
            guard let slot = slots[id], let webView = slot.webView,
                  webView.bounds.width > 0, webView.bounds.height > 0
            else { continue }
            let hidden = slot.isHidden
            slot.isHidden = false
            let config = WKSnapshotConfiguration()
            config.rect = webView.bounds
            config.snapshotWidth = NSNumber(value: Double(webView.bounds.width * scale / max(backing, 1)))
            webView.takeSnapshot(with: config) { [weak self, weak slot] image, _ in
                MainActor.assumeIsolated {
                    if hidden, let slot, self?.slots[id] === slot, slot.wantsHidden {
                        slot.isHidden = true
                    }
                    guard let png = image.flatMap(gpuPicturePng) else { return }
                    png.withUnsafeBytes { raw in
                        gpuPageWebViewPicture(token, id, raw.bindMemory(to: UInt8.self))
                    }
                }
            }
        }
    }

    func clearAll() {
        for slot in slots.values {
            slot.webView?.removeFromSuperview()
            slot.removeFromSuperview()
        }
        slots.removeAll()
        orderedIds.removeAll()
        isHidden = true
    }

    func sync(json: String) -> Bool {
        guard let data = json.data(using: .utf8),
              let specs = try? JSONDecoder().decode([GpuWebViewMountSpec].self, from: data),
              !specs.isEmpty
        else {
            clearAll()
            return false
        }
        if isHidden {
            isHidden = false
        }

        CATransaction.begin()
        CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }

        let nextIds = specs.map(\.webId)
        for (id, slot) in slots where !nextIds.contains(id) {
            slot.webView?.removeFromSuperview()
            slot.removeFromSuperview()
            slots.removeValue(forKey: id)
        }
        orderedIds = nextIds

        defer {
            // Later in paint order stacks above, as the page draws them.
            let order = subviews.compactMap { ($0 as? GpuWebViewSlot)?.webId }
            if order != nextIds.filter({ slots[$0] != nil }) {
                for id in nextIds {
                    if let slot = slots[id] { addSubview(slot, positioned: .above, relativeTo: nil) }
                }
            }
        }

        for spec in specs {
            let slot = slots[spec.webId] ?? {
                let created = GpuWebViewSlot(webId: spec.webId)
                slots[spec.webId] = created
                addSubview(created)
                return created
            }()
            slot.attachIfNeeded(token: token)
            if let dark = spec.dark, let webView = slot.webView,
               slot.appliedDark.map({ $0 != (dark, ObjectIdentifier(webView)) }) ?? true {
                slot.appliedDark = (dark, ObjectIdentifier(webView))
                LxAppAppearanceRegistry.apply(dark, to: webView)
            }

            let bx = spec.bounds.count == 4 ? spec.bounds[0] : 0
            let by = spec.bounds.count == 4 ? spec.bounds[1] : 0
            let bw = spec.bounds.count == 4 ? max(1, spec.bounds[2]) : 1
            let bh = spec.bounds.count == 4 ? max(1, spec.bounds[3]) : 1

            let vx = spec.visibleBounds.count == 4 ? spec.visibleBounds[0] : bx
            let vy = spec.visibleBounds.count == 4 ? spec.visibleBounds[1] : by
            let vw = spec.visibleBounds.count == 4 ? max(0, spec.visibleBounds[2]) : 0
            let vh = spec.visibleBounds.count == 4 ? max(0, spec.visibleBounds[3]) : 0

            let hidden = spec.degraded || vw <= 0.01 || vh <= 0.01
            slot.wantsHidden = hidden
            slot.isHidden = hidden
            slot.accessibilitySuppressed = spec.accessibilityHidden || spec.degraded
            slot.setAccessibilityElement(!slot.accessibilitySuppressed)

            if !hidden {
                let clipRect = CGRect(x: vx, y: vy, width: vw, height: vh)
                if slot.frame != clipRect {
                    slot.frame = clipRect
                }
                let innerFrame = CGRect(x: bx - clipRect.minX, y: by - clipRect.minY, width: bw, height: bh)
                if let wv = slot.webView {
                    if wv.frame != innerFrame {
                        wv.frame = innerFrame
                    }
                    if wv.isHidden {
                        wv.isHidden = false
                    }
                }
                slot.layer?.mask = buildMaskChain(spec: spec, clipOrigin: clipRect.origin)
            }
        }
        return !slots.isEmpty
    }
}
#endif
