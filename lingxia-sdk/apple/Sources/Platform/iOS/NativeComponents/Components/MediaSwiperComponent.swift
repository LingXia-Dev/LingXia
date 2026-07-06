import Foundation
import OSLog

#if os(iOS)
import UIKit

private let swiperLog = OSLog(subsystem: "LingXia", category: "MediaSwiper")

@MainActor
final class MediaSwiperComponentFactory: LxNativeComponentFactory {
    func make(
        id: String,
        initialProps: [String: Any],
        eventSink: @escaping ([String: Any]) -> Void
    ) -> LxNativeComponent {
        MediaSwiperComponent(id: id, initialProps: initialProps, eventSink: eventSink)
    }
}

@MainActor
final class MediaSwiperComponent: NSObject, LxNativeComponent, UIScrollViewDelegate {
    let id: String
    let view: UIView

    private let scrollView: UIScrollView
    private let pageContainer: UIView
    private let dotsStack: UIStackView
    private var dotsHorizontalConstraints: [NSLayoutConstraint] = []
    private var dotsVerticalConstraints: [NSLayoutConstraint] = []
    private var pages: [Int: SwiperPageView] = [:]
    private let eventSink: ([String: Any]) -> Void

    private var config = MediaSwiperConfig()
    private var currentIndex: Int = 0
    private var lastSettledIndex: Int = 0
    private var ignoreNextSelection: Bool = false
    private var pendingTransitionPrevious: Int?
    private var pendingTransitionSource: String?
    private var autoplayTimer: Timer?
    private var lastBounds: CGRect = .zero

    init(id: String, initialProps: [String: Any], eventSink: @escaping ([String: Any]) -> Void) {
        self.id = id
        self.eventSink = eventSink

        let root = UIView()
        root.backgroundColor = .black
        root.clipsToBounds = true

        let scroll = UIScrollView()
        scroll.isPagingEnabled = true
        scroll.showsHorizontalScrollIndicator = false
        scroll.showsVerticalScrollIndicator = false
        scroll.bounces = false
        scroll.alwaysBounceHorizontal = false
        scroll.alwaysBounceVertical = false
        scroll.contentInsetAdjustmentBehavior = .never
        // Use autoresizingMask so scroll.frame stays in sync with root.bounds the
        // moment we assign root.frame. Auto Layout constraints would defer this to
        // the next layout pass and leave scroll at zero size when relayoutPages runs.
        scroll.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        root.addSubview(scroll)

        let pageContainer = UIView()
        scroll.addSubview(pageContainer)

        let dots = UIStackView()
        dots.axis = .horizontal
        dots.spacing = 8
        dots.alignment = .center
        dots.translatesAutoresizingMaskIntoConstraints = false
        dots.isHidden = true
        root.addSubview(dots)

        let dotsHorizontalConstraints = [
            dots.bottomAnchor.constraint(equalTo: root.bottomAnchor, constant: -12),
            dots.centerXAnchor.constraint(equalTo: root.centerXAnchor),
        ]
        let dotsVerticalConstraints = [
            dots.trailingAnchor.constraint(equalTo: root.trailingAnchor, constant: -12),
            dots.centerYAnchor.constraint(equalTo: root.centerYAnchor),
        ]
        NSLayoutConstraint.activate(dotsHorizontalConstraints)

        self.scrollView = scroll
        self.pageContainer = pageContainer
        self.dotsStack = dots
        self.dotsHorizontalConstraints = dotsHorizontalConstraints
        self.dotsVerticalConstraints = dotsVerticalConstraints
        self.view = root

        super.init()
        scroll.delegate = self
        update(props: initialProps)
    }

    func mount(in host: UIView) {
        host.addSubview(view)
    }

    func update(props: [String: Any]) {
        let previousItems = config.items
        let previousIndex = currentIndex
        let priorItem = previousItems.indices.contains(previousIndex) ? previousItems[previousIndex] : nil
        let previousPeekPrevious = config.peekPrevious
        let previousPeekNext = config.peekNext
        let previousDirection = config.direction
        let next = MediaSwiperConfig.parse(props: props, previous: config)
        let itemsChanged = previousItems != next.items
        let layoutChanged = !itemsChanged && (
            previousPeekPrevious != next.peekPrevious ||
            previousPeekNext != next.peekNext ||
            previousDirection != next.direction
        )
        config = next

        scrollView.isScrollEnabled = next.swipeEnabled

        if itemsChanged {
            currentIndex = resolveIndexForItemsChange(
                next: next,
                previousItems: previousItems,
                previousIndex: previousIndex,
                priorItem: priorItem
            )
            lastSettledIndex = currentIndex
            rebuildPages()
            relayoutPages()
            // Non-animated jump fires no scroll-end callback on UIKit, so don't arm
            // ignoreNextSelection — leaving it set would silently swallow the next
            // user-driven change emit.
            scrollToCurrent(animated: false)
            refreshVisiblePagesPlayback()
            updateDots()
        } else {
            for (_, page) in pages {
                page.applyConfig(next, swiperId: id)
            }
            if layoutChanged {
                // Peek/direction changes redo the page positioning math without
                // rebuilding pages or losing video state.
                relayoutPages()
                scrollToCurrent(animated: false)
                updateDots()
            }
            if let controlled = next.index {
                let resolved = clampIndex(controlled, count: next.items.count)
                if resolved != currentIndex {
                    currentIndex = resolved
                    lastSettledIndex = resolved
                    scrollToCurrent(animated: false)
                    refreshVisiblePagesPlayback()
                    updateDots()
                }
            }
        }

        scheduleAutoplay()
    }

    func setFrame(_ frame: CGRect) {
        if !view.frame.equalTo(frame) {
            view.frame = frame
        }
        // scroll uses autoresizingMask, but autoresize only kicks in during the
        // superview's layout pass — explicitly mirror root's bounds onto scroll so
        // relayoutPages reads non-zero bounds even before any layout pass runs.
        scrollView.frame = CGRect(origin: .zero, size: frame.size)
        if lastBounds.size != frame.size {
            lastBounds = frame
            relayoutPages()
            scrollToCurrent(animated: false)
        }
    }

    func focus() {}
    func blur() {}

    func handleCommand(name: String, params: [String: Any]?) {
        switch name {
        case "next":
            goBy(delta: 1, source: "api")
        case "previous":
            goBy(delta: -1, source: "api")
        case "goToIndex":
            guard let index = (params?["index"] as? NSNumber)?.intValue else { return }
            if index < 0 || index >= config.items.count { return }
            goTo(target: index, source: "api", animated: config.animation != "none")
        default:
            break
        }
    }

    func unmount() {
        stopAutoplay()
        for (_, page) in pages {
            page.detach()
        }
        pages.removeAll()
        view.removeFromSuperview()
    }

    // MARK: - Pages

    private func rebuildPages() {
        for (_, page) in pages { page.detach() }
        pages.removeAll()
        for (index, item) in config.items.enumerated() {
            let page = SwiperPageView(
                index: index,
                item: item,
                config: config,
                swiperId: id,
                eventSink: { [weak self] event, detail in
                    self?.handlePageEvent(event: event, detail: detail)
                }
            )
            page.attach(to: pageContainer)
            pages[index] = page
        }
    }

    private func relayoutPages() {
        // setFrame already synced scrollView.frame; either side is reliable here.
        let size = view.bounds.size
        guard size.width > 0, size.height > 0 else { return }
        let count = config.items.count
        let horizontal = config.direction != "vertical"
        let pageStride = pageStrideAlongAxis(size: size, horizontal: horizontal)
        let leadOffset = config.peekPrevious
        let trailingPeek = config.peekNext
        let pageBreadth = horizontal ? size.height : size.width

        let contentMain = pageStride * CGFloat(count) + leadOffset + trailingPeek
        scrollView.contentSize = horizontal
            ? CGSize(width: contentMain, height: size.height)
            : CGSize(width: size.width, height: contentMain)
        pageContainer.frame = CGRect(origin: .zero, size: scrollView.contentSize)
        // Native UIScrollView paging snaps at bounds-width intervals only; with peek
        // the page stride is smaller, so opt out of native paging and snap manually
        // via scrollViewWillEndDragging.
        let hasPeek = config.peekPrevious > 0 || config.peekNext > 0
        scrollView.isPagingEnabled = !hasPeek
        scrollView.decelerationRate = hasPeek ? .fast : .normal
        for (index, page) in pages {
            let frame: CGRect
            if horizontal {
                frame = CGRect(
                    x: leadOffset + pageStride * CGFloat(index),
                    y: 0,
                    width: pageStride,
                    height: pageBreadth
                )
            } else {
                frame = CGRect(
                    x: 0,
                    y: leadOffset + pageStride * CGFloat(index),
                    width: pageBreadth,
                    height: pageStride
                )
            }
            page.setFrame(frame)
        }
    }

    /// Stride between consecutive page origins along the active axis. Equals the
    /// scrollView's bounds dimension when peek is zero, shrinking by `peek` on
    /// either side so adjacent pages remain partially visible.
    private func pageStrideAlongAxis(size: CGSize, horizontal: Bool) -> CGFloat {
        let main = horizontal ? size.width : size.height
        let stride = main - config.peekPrevious - config.peekNext
        return max(1, stride)
    }

    private func scrollToCurrent(animated: Bool) {
        let size = view.bounds.size
        guard size.width > 0, size.height > 0 else { return }
        let horizontal = config.direction != "vertical"
        let stride = pageStrideAlongAxis(size: size, horizontal: horizontal)
        // Pages start at `peekPrevious`, so scrolling to page i means the leading
        // edge of page i lands at viewport offset 0; the previous-page peek is then
        // the visible slice of page (i-1) at position 0..peekPrevious.
        let target: CGPoint
        if horizontal {
            target = CGPoint(x: stride * CGFloat(currentIndex), y: 0)
        } else {
            target = CGPoint(x: 0, y: stride * CGFloat(currentIndex))
        }
        scrollView.setContentOffset(target, animated: animated)
        if !animated {
            // setContentOffset(animated:false) does not fire scrollViewDidEndDecelerating;
            // settle visibility immediately so onVisible/onHidden runs in sync with the snap.
            refreshVisiblePagesPlayback()
        }
    }

    private func refreshVisiblePagesPlayback() {
        for (idx, page) in pages {
            if idx == currentIndex {
                page.onVisible()
            } else {
                page.onHidden()
            }
        }
    }

    // MARK: - Index transitions

    private func clampIndex(_ value: Int, count: Int) -> Int {
        let last = max(0, count - 1)
        return min(max(0, value), last)
    }

    private func resolveInitialIndex(_ cfg: MediaSwiperConfig) -> Int {
        let raw = cfg.index ?? cfg.initialIndex
        return clampIndex(raw, count: cfg.items.count)
    }

    private func resolveIndexForItemsChange(
        next: MediaSwiperConfig,
        previousItems: [MediaSwiperItem],
        previousIndex: Int,
        priorItem: MediaSwiperItem?
    ) -> Int {
        if let controlled = next.index {
            return clampIndex(controlled, count: next.items.count)
        }
        if let priorItem, !previousItems.isEmpty,
           let matched = next.items.firstIndex(where: { $0.id == priorItem.id }) {
            return matched
        }
        return resolveInitialIndex(next)
    }

    private func goBy(delta: Int, source: String) {
        let count = config.items.count
        if count == 0 { return }
        let target = currentIndex + delta
        if target < 0 || target >= count {
            if config.loop && count > 1 {
                let wrapped = delta > 0 ? 0 : count - 1
                goTo(target: wrapped, source: source, animated: config.animation != "none")
            } else {
                emitEndReached(at: currentIndex, source: source)
                if source == "autoplay" { stopAutoplay() }
            }
            return
        }
        goTo(target: target, source: source, animated: config.animation != "none")
        if source == "autoplay" && !config.loop && target == count - 1 {
            emitEndReached(at: target, source: source)
            stopAutoplay()
        }
    }

    private func goTo(target: Int, source: String, animated: Bool) {
        if target == currentIndex { return }
        let previous = currentIndex
        currentIndex = target
        // If we can't actually scroll yet (e.g. mounted but layout hasn't run), the
        // animated path's scroll-end callback won't fire and transitionend would be
        // lost. Force the non-animated path in that case so every change is paired
        // with a transitionend.
        let scrollSize = view.bounds.size
        let canAnimate = animated && scrollSize.width > 0 && scrollSize.height > 0
        if canAnimate {
            // Arm ignoreNextSelection so the synthesized scroll-end callback emits
            // transitionend without re-emitting change.
            pendingTransitionPrevious = previous
            pendingTransitionSource = source
            ignoreNextSelection = true
        } else {
            pendingTransitionPrevious = nil
            pendingTransitionSource = nil
        }
        emitChange(index: target, previous: previous, source: source)
        scrollToCurrent(animated: canAnimate)
        if !canAnimate {
            lastSettledIndex = target
            emitTransitionEnd(index: target, previous: previous, source: source)
            refreshVisiblePagesPlayback()
        }
        updateDots()
        scheduleAutoplay()
    }

    // MARK: - Events

    private func emitChange(index: Int, previous: Int, source: String) {
        let item = config.items.indices.contains(index) ? config.items[index].toPayload() : [:]
        emit(event: "change", detail: [
            "index": index,
            "previousIndex": previous,
            "item": item,
            "source": source,
        ])
    }

    private func emitTransitionEnd(index: Int, previous: Int, source: String) {
        let item = config.items.indices.contains(index) ? config.items[index].toPayload() : [:]
        emit(event: "transitionend", detail: [
            "index": index,
            "previousIndex": previous,
            "item": item,
            "source": source,
        ])
    }

    private func emitEndReached(at index: Int, source: String) {
        let item = config.items.indices.contains(index) ? config.items[index].toPayload() : [:]
        emit(event: "endreached", detail: [
            "index": index,
            "item": item,
            "source": source,
        ])
    }

    private func emit(event: String, detail: [String: Any]) {
        eventSink([
            "event": event,
            "detail": detail,
        ])
    }

    private func handlePageEvent(event: String, detail: [String: Any]) {
        emit(event: event, detail: detail)
    }

    // MARK: - Dots

    private func updateDots() {
        let cfg = config
        guard cfg.dotsEnabled, cfg.items.count > 1 else {
            dotsStack.isHidden = true
            return
        }
        dotsStack.isHidden = false
        let vertical = cfg.direction == "vertical"
        dotsStack.axis = vertical ? .vertical : .horizontal
        dotsStack.spacing = vertical ? 8 : 8
        NSLayoutConstraint.deactivate(vertical ? dotsHorizontalConstraints : dotsVerticalConstraints)
        NSLayoutConstraint.activate(vertical ? dotsVerticalConstraints : dotsHorizontalConstraints)

        let needed = cfg.items.count
        while dotsStack.arrangedSubviews.count > needed {
            let last = dotsStack.arrangedSubviews.last!
            dotsStack.removeArrangedSubview(last)
            last.removeFromSuperview()
        }
        while dotsStack.arrangedSubviews.count < needed {
            let dot = UIView()
            dot.translatesAutoresizingMaskIntoConstraints = false
            NSLayoutConstraint.activate([
                dot.widthAnchor.constraint(equalToConstant: 6),
                dot.heightAnchor.constraint(equalToConstant: 6),
            ])
            dot.layer.cornerRadius = 3
            dotsStack.addArrangedSubview(dot)
        }
        for (i, dot) in dotsStack.arrangedSubviews.enumerated() {
            dot.backgroundColor = (i == currentIndex) ? cfg.dotsActiveColor : cfg.dotsColor
        }
    }

    // MARK: - Autoplay

    private func scheduleAutoplay() {
        stopAutoplay()
        guard config.autoplay, config.items.count > 1 else {
            return
        }
        if !config.loop && currentIndex >= config.items.count - 1 {
            return
        }
        let interval = max(0.5, Double(config.interval) / 1000.0)
        autoplayTimer = Timer.scheduledTimer(withTimeInterval: interval, repeats: false) { [weak self] _ in
            Task { @MainActor [weak self] in
                guard let self else { return }
                self.goBy(delta: 1, source: "autoplay")
            }
        }
    }

    private func stopAutoplay() {
        autoplayTimer?.invalidate()
        autoplayTimer = nil
    }

    // MARK: - UIScrollViewDelegate

    func scrollViewDidEndDecelerating(_ scrollView: UIScrollView) {
        finishScroll()
    }

    func scrollViewDidEndDragging(_ scrollView: UIScrollView, willDecelerate decelerate: Bool) {
        // Slow drag releases between pages snap-settle without firing didEndDecelerating;
        // catch that path so change/transitionend still emit.
        if !decelerate {
            finishScroll()
        }
    }

    func scrollViewDidEndScrollingAnimation(_ scrollView: UIScrollView) {
        finishScroll()
    }

    func scrollViewWillEndDragging(
        _ scrollView: UIScrollView,
        withVelocity velocity: CGPoint,
        targetContentOffset: UnsafeMutablePointer<CGPoint>
    ) {
        // Native paging only snaps at bounds-sized intervals. With peek the stride is
        // smaller; intercept the deceleration target and round to the nearest page
        // index along the active axis.
        guard config.peekPrevious > 0 || config.peekNext > 0 else { return }
        let size = scrollView.bounds.size
        guard size.width > 0, size.height > 0 else { return }
        let horizontal = config.direction != "vertical"
        let stride = pageStrideAlongAxis(size: size, horizontal: horizontal)
        let count = config.items.count
        if horizontal {
            let raw = targetContentOffset.pointee.x / stride
            let snappedIndex = clampIndex(Int(round(raw)), count: count)
            targetContentOffset.pointee.x = stride * CGFloat(snappedIndex)
        } else {
            let raw = targetContentOffset.pointee.y / stride
            let snappedIndex = clampIndex(Int(round(raw)), count: count)
            targetContentOffset.pointee.y = stride * CGFloat(snappedIndex)
        }
    }

    private func finishScroll() {
        let size = scrollView.bounds.size
        guard size.width > 0, size.height > 0 else { return }
        let horizontal = config.direction != "vertical"
        let stride = pageStrideAlongAxis(size: size, horizontal: horizontal)
        let offset = horizontal ? scrollView.contentOffset.x : scrollView.contentOffset.y
        let landed = Int(round(offset / stride))
        let resolved = clampIndex(landed, count: config.items.count)

        if ignoreNextSelection {
            ignoreNextSelection = false
            if let prev = pendingTransitionPrevious, let source = pendingTransitionSource, resolved != prev {
                lastSettledIndex = resolved
                emitTransitionEnd(index: resolved, previous: prev, source: source)
                pendingTransitionPrevious = nil
                pendingTransitionSource = nil
            } else {
                lastSettledIndex = resolved
            }
            refreshVisiblePagesPlayback()
            updateDots()
            return
        }

        let previous = currentIndex
        if resolved != previous {
            currentIndex = resolved
            lastSettledIndex = resolved
            emitChange(index: resolved, previous: previous, source: "touch")
            emitTransitionEnd(index: resolved, previous: previous, source: "touch")
            refreshVisiblePagesPlayback()
            updateDots()
            scheduleAutoplay()
        }
    }
}

// MARK: - Config

private struct MediaSwiperItem: Equatable {
    enum Kind: String { case image, video }
    let id: String
    let kind: Kind
    let src: String
    let poster: String?
    let controls: Bool?
    let muted: Bool?

    func toPayload() -> [String: Any] {
        var dict: [String: Any] = [
            "id": id,
            "type": kind.rawValue,
            "src": src,
        ]
        if let poster { dict["poster"] = poster }
        if let controls { dict["controls"] = controls }
        if let muted { dict["muted"] = muted }
        return dict
    }
}

private struct MediaSwiperConfig: Equatable {
    var items: [MediaSwiperItem] = []
    var index: Int? = nil
    var initialIndex: Int = 0
    var loop: Bool = false
    var autoplay: Bool = false
    var interval: Int = 5000
    var animation: String = "slide"
    var direction: String = "horizontal"
    var rotate: Int = 0
    var objectFit: LxMediaObjectFit = .cover
    var controls: Bool = false
    var muted: Bool = true
    var dotsEnabled: Bool = false
    var dotsColor: UIColor = UIColor(white: 1, alpha: 0.4)
    var dotsActiveColor: UIColor = .white
    var swipeEnabled: Bool = true
    /// Peek values in points for the previous/next pages along the swipe axis.
    /// Non-zero values disable native paging snap and use custom snap math so the
    /// stride (pageWidth) is `bounds.size - previous - next` instead of the full
    /// scrollView dimension.
    var peekPrevious: CGFloat = 0
    var peekNext: CGFloat = 0

    static func parse(props: [String: Any], previous: MediaSwiperConfig) -> MediaSwiperConfig {
        var next = previous

        if let raw = props["items"] as? [Any] {
            next.items = raw.enumerated().compactMap { index, entry in
                guard let map = entry as? [String: Any] else { return nil }
                guard let typeRaw = map["type"] as? String,
                      let kind = MediaSwiperItem.Kind(rawValue: typeRaw),
                      let src = (map["src"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines),
                      !src.isEmpty
                else { return nil }
                let id = (map["id"] as? String) ?? "\(typeRaw):\(src):\(index)"
                let poster = map["poster"] as? String
                let controls = map["controls"] as? Bool
                let muted = map["muted"] as? Bool
                return MediaSwiperItem(
                    id: id,
                    kind: kind,
                    src: src,
                    poster: poster,
                    controls: controls,
                    muted: muted
                )
            }
        }

        if let n = props["index"] as? NSNumber { next.index = n.intValue } else if props["index"] is NSNull { next.index = nil }
        if let n = props["initialIndex"] as? NSNumber { next.initialIndex = n.intValue }
        if let v = props["loop"] as? Bool { next.loop = v }
        if let v = props["autoplay"] as? Bool { next.autoplay = v }
        if let n = props["interval"] as? NSNumber { next.interval = max(500, n.intValue) }
        if let s = props["animation"] as? String, s == "none" || s == "slide" { next.animation = s }
        if let s = props["direction"] as? String, s == "vertical" || s == "horizontal" { next.direction = s }
        if let n = props["contentRotate"] as? NSNumber {
            let r = n.intValue
            next.rotate = (r == 0 || r == 90 || r == 180 || r == 270) ? r : 0
        }
        if let s = props["objectFit"] as? String, let fit = LxMediaObjectFit(rawValue: s.lowercased()) {
            next.objectFit = fit
        }
        if let v = props["controls"] as? Bool { next.controls = v }
        if let v = props["muted"] as? Bool { next.muted = v }
        if let v = props["swipeEnabled"] as? Bool { next.swipeEnabled = v }

        switch props["dots"] {
        case let v as Bool:
            next.dotsEnabled = v
        case let dict as [String: Any]:
            next.dotsEnabled = true
            if let s = dict["color"] as? String, let c = UIColor.fromHexOrName(s) { next.dotsColor = c }
            if let s = dict["activeColor"] as? String, let c = UIColor.fromHexOrName(s) { next.dotsActiveColor = c }
        default:
            break
        }

        switch props["peek"] {
        case let n as NSNumber:
            let value = max(0, CGFloat(n.doubleValue))
            next.peekPrevious = value
            next.peekNext = value
        case let dict as [String: Any]:
            if let prev = (dict["previous"] as? NSNumber)?.doubleValue {
                next.peekPrevious = max(0, CGFloat(prev))
            }
            if let nx = (dict["next"] as? NSNumber)?.doubleValue {
                next.peekNext = max(0, CGFloat(nx))
            }
        case is NSNull:
            next.peekPrevious = 0
            next.peekNext = 0
        default:
            break
        }

        return next
    }
}

// MARK: - Page

@MainActor
private final class SwiperPageView {
    let index: Int
    private let item: MediaSwiperItem
    private var config: MediaSwiperConfig
    private let swiperId: String
    private let eventSink: (_ event: String, _ detail: [String: Any]) -> Void

    private let container: UIView
    private var imageView: UIImageView?
    private var imageTask: URLSessionDataTask?
    private var imageRequestToken: UUID = UUID()
    private var player: LxMediaPlayer?
    private var tapRecognizer: UITapGestureRecognizer?
    private var lastRenderKey: String?

    init(
        index: Int,
        item: MediaSwiperItem,
        config: MediaSwiperConfig,
        swiperId: String,
        eventSink: @escaping (_ event: String, _ detail: [String: Any]) -> Void
    ) {
        self.index = index
        self.item = item
        self.config = config
        self.swiperId = swiperId
        self.eventSink = eventSink
        self.container = UIView()
        self.container.backgroundColor = .black
        self.container.clipsToBounds = true
    }

    func attach(to host: UIView) {
        if container.superview !== host {
            host.addSubview(container)
        }
        bind()
    }

    func detach() {
        imageTask?.cancel()
        imageTask = nil
        player?.detach()
        player = nil
        imageView = nil
        for sub in container.subviews { sub.removeFromSuperview() }
        container.removeFromSuperview()
    }

    func setFrame(_ frame: CGRect) {
        container.frame = frame
        imageView?.frame = container.bounds
        applyImageRotation()
        player?.setFrame(container.bounds)
    }

    func applyConfig(_ config: MediaSwiperConfig, swiperId: String) {
        self.config = config
        let renderKey = currentRenderKey()
        if renderKey == lastRenderKey {
            // Only the props that don't require rebind changed (e.g. unchanged for this page).
            // For videos, push controls/muted/objectFit/rotate updates onto the live player.
            if item.kind == .video, let player {
                var cfg = LxMediaPlayerConfig()
                cfg.controls = item.controls ?? config.controls
                cfg.muted = item.muted ?? config.muted
                cfg.objectFit = config.objectFit
                cfg.rotateDegrees = config.rotate
                player.update(config: cfg)
            } else {
                applyImageRotation()
                applyImageContentMode()
            }
            return
        }
        // Render-affecting props changed for this page; rebuild.
        bind()
    }

    func onVisible() {
        // Spec: current item plays (controls=false default + muted=true default means autoplay
        // is the only path to playback). Hidden videos pause via onHidden.
        if item.kind == .video {
            player?.handle(command: .play)
        }
    }

    func onHidden() {
        if item.kind == .video {
            player?.handle(command: .pause)
        }
    }

    // MARK: - Private

    private func currentRenderKey() -> String {
        let effectiveControls = item.controls ?? config.controls
        let effectiveMuted = item.muted ?? config.muted
        return [
            String(index),
            item.id,
            item.kind.rawValue,
            item.src,
            item.poster ?? "",
            String(effectiveControls),
            String(effectiveMuted),
            config.objectFit.rawValue,
            String(config.rotate),
        ].joined(separator: "|")
    }

    private func bind() {
        let renderKey = currentRenderKey()
        if lastRenderKey == renderKey { return }
        // Tear down previous content before binding new.
        imageTask?.cancel()
        imageTask = nil
        player?.detach()
        player = nil
        imageView = nil
        for sub in container.subviews { sub.removeFromSuperview() }
        lastRenderKey = renderKey

        switch item.kind {
        case .image:
            bindImage()
        case .video:
            bindVideo()
        }
        installTapGesture()
    }

    private func bindImage() {
        let image = UIImageView(frame: container.bounds)
        image.backgroundColor = .black
        image.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        image.clipsToBounds = true
        container.addSubview(image)
        imageView = image
        applyImageContentMode()
        applyImageRotation()
        loadImage(into: image)
    }

    private func applyImageContentMode() {
        guard let imageView else { return }
        switch config.objectFit {
        case .cover: imageView.contentMode = .scaleAspectFill
        case .contain, .fit: imageView.contentMode = .scaleAspectFit
        case .fill: imageView.contentMode = .scaleToFill
        }
    }

    private func applyImageRotation() {
        guard let imageView else { return }
        let degrees = CGFloat(config.rotate)
        if degrees == 0 {
            imageView.transform = .identity
        } else {
            imageView.transform = CGAffineTransform(rotationAngle: degrees * .pi / 180)
        }
    }

    private func bindVideo() {
        let player = LxMediaPlayer(eventSink: { [weak self] payload in
            self?.handleMediaPayload(payload)
        })
        player.attach(to: container)
        player.setFrame(container.bounds)
        var cfg = LxMediaPlayerConfig()
        if let url = MediaSwiperURL.resolve(item.src) {
            cfg.src = url
        }
        if let posterStr = item.poster, let posterURL = MediaSwiperURL.resolve(posterStr) {
            cfg.poster = posterURL
        }
        cfg.controls = item.controls ?? config.controls
        cfg.muted = item.muted ?? config.muted
        cfg.loop = false
        cfg.autoplay = false
        cfg.objectFit = config.objectFit
        cfg.rotateDegrees = config.rotate
        cfg.live = false
        cfg.progressBar = (item.controls ?? config.controls)
        player.update(config: cfg)
        self.player = player
    }

    private func installTapGesture() {
        if let existing = tapRecognizer {
            container.removeGestureRecognizer(existing)
            tapRecognizer = nil
        }
        // When video controls are enabled, LxMediaPlayer owns its own tap surface for showing
        // controls; we must not bubble those taps as swiper onTap. For images and controls-off
        // videos, attach a tap recognizer on the page container.
        if item.kind == .video && (item.controls ?? config.controls) {
            return
        }
        let tap = UITapGestureRecognizer(target: self, action: #selector(handleTap))
        tap.cancelsTouchesInView = false
        container.addGestureRecognizer(tap)
        tapRecognizer = tap
    }

    @objc private func handleTap() {
        eventSink("tap", [
            "index": index,
            "item": item.toPayload(),
        ])
    }

    private func handleMediaPayload(_ payload: [String: Any]) {
        guard let event = payload["event"] as? String else { return }
        switch event {
        case "ended":
            eventSink("videoended", [
                "index": index,
                "item": item.toPayload(),
            ])
        case "error":
            let detail = payload["detail"] as? [String: Any]
            let code = (detail?["code"] as? String) ?? "unknown"
            let message = (detail?["message"] as? String) ?? "video error"
            eventSink("error", [
                "index": index,
                "item": item.toPayload(),
                "code": MediaSwiperError.normalize(code),
                "message": message,
            ])
        default:
            break
        }
    }

    private func loadImage(into target: UIImageView) {
        let token = UUID()
        imageRequestToken = token
        guard let url = MediaSwiperURL.resolve(item.src) else {
            LXLog.error("[page \(index)] loadImage resolve nil for src=\(item.src)", category: "MediaSwiper")
            emitError(code: "not_found", message: "image source not found")
            return
        }
        if url.isFileURL {
            let path = url.path
            if !FileManager.default.fileExists(atPath: path) {
                LXLog.error("[page \(index)] loadImage file missing path=\(url.lastPathComponent)", category: "MediaSwiper")
                emitError(code: "not_found", message: "image file does not exist")
                return
            }
            DispatchQueue.global(qos: .userInitiated).async { [weak self] in
                guard let data = try? Data(contentsOf: url), let image = UIImage(data: data) else {
                    Task { @MainActor [weak self] in
                        guard let self, self.imageRequestToken == token else { return }
                        LXLog.error("[page \(self.index)] loadImage decode failed", category: "MediaSwiper")
                        self.emitError(code: "decode", message: "image decode failed")
                    }
                    return
                }
                Task { @MainActor [weak self] in
                    guard let self, self.imageRequestToken == token else { return }
                    target.image = image
                }
            }
            return
        }
        let task = URLSession.shared.dataTask(with: url) { [weak self] data, response, error in
            Task { @MainActor [weak self] in
                guard let self, self.imageRequestToken == token else { return }
                if let error = error as NSError? {
                    let code = error.domain == NSURLErrorDomain ? "network" : "unknown"
                    self.emitError(code: code, message: error.localizedDescription)
                    return
                }
                if let httpResponse = response as? HTTPURLResponse, !(200..<300).contains(httpResponse.statusCode) {
                    self.emitError(code: "network", message: "HTTP \(httpResponse.statusCode)")
                    return
                }
                guard let data, let image = UIImage(data: data) else {
                    self.emitError(code: "decode", message: "image decode failed")
                    return
                }
                target.image = image
            }
        }
        imageTask = task
        task.resume()
    }

    private func emitError(code: String, message: String) {
        eventSink("error", [
            "index": index,
            "item": item.toPayload(),
            "code": MediaSwiperError.normalize(code),
            "message": message,
        ])
    }
}

// MARK: - Helpers

private enum MediaSwiperURL {
    @MainActor
    static func resolve(_ src: String) -> URL? {
        let trimmed = src.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { return nil }
        if trimmed.hasPrefix("http://") || trimmed.hasPrefix("https://") {
            return URL(string: trimmed)
        }
        let current = getCurrentLxApp()
        let appId = current.appid.toString()
        let resolvedRaw = resolveLxUri(appId, trimmed)?.toString()
        let resolved = resolvedRaw ?? trimmed
        // Prefer file URLs from raw paths to avoid URL(string:) failing on unencoded
        // characters (spaces, parentheses, non-ASCII) that are valid in iOS file paths.
        if let path = resolved.strippingFileScheme(), path.hasPrefix("/") {
            return URL(fileURLWithPath: path)
        }
        if resolved.hasPrefix("/") {
            return URL(fileURLWithPath: resolved)
        }
        if let url = URL(string: resolved), url.scheme != nil {
            return url
        }
        return nil
    }
}

private extension String {
    /// If self starts with "file://", returns the path remainder; otherwise nil.
    func strippingFileScheme() -> String? {
        guard self.hasPrefix("file://") else { return nil }
        return String(self.dropFirst("file://".count))
    }
}

private enum MediaSwiperError {
    static let known: Set<String> = [
        "not_found", "network", "decode", "unsupported_format", "permission_denied", "unknown",
    ]
    static func normalize(_ code: String) -> String {
        let lower = code.lowercased()
        return known.contains(lower) ? lower : "unknown"
    }
}

private extension UIColor {
    static func fromHexOrName(_ raw: String) -> UIColor? {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { return nil }
        if trimmed.hasPrefix("#") {
            let hex = String(trimmed.dropFirst())
            return colorFromHex(hex)
        }
        switch trimmed.lowercased() {
        case "white": return .white
        case "black": return .black
        case "red": return .red
        case "green": return .green
        case "blue": return .blue
        case "clear", "transparent": return .clear
        default: return colorFromHex(trimmed)
        }
    }

    private static func colorFromHex(_ hex: String) -> UIColor? {
        var value: UInt64 = 0
        let scanner = Scanner(string: hex)
        guard scanner.scanHexInt64(&value) else { return nil }
        switch hex.count {
        case 6:
            let r = CGFloat((value & 0xFF0000) >> 16) / 255
            let g = CGFloat((value & 0x00FF00) >> 8) / 255
            let b = CGFloat(value & 0x0000FF) / 255
            return UIColor(red: r, green: g, blue: b, alpha: 1)
        case 8:
            // CSS-compatible RGBA ordering (#RRGGBBAA), matching the JS string
            // contracts users pass via the `dots` prop.
            let r = CGFloat((value & 0xFF000000) >> 24) / 255
            let g = CGFloat((value & 0x00FF0000) >> 16) / 255
            let b = CGFloat((value & 0x0000FF00) >> 8) / 255
            let a = CGFloat(value & 0x000000FF) / 255
            return UIColor(red: r, green: g, blue: b, alpha: a)
        default:
            return nil
        }
    }
}

#endif
