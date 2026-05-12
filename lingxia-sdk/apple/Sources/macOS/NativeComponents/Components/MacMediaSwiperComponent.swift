#if os(macOS)
import Foundation
import AppKit
import CLingXiaRustAPI

@MainActor
final class MacMediaSwiperComponentFactory: MacNativeComponentFactory {
    func make(
        id: String,
        initialProps: [String: Any],
        eventSink: @escaping ([String: Any]) -> Void
    ) -> MacNativeComponent {
        MacMediaSwiperComponent(id: id, initialProps: initialProps, eventSink: eventSink)
    }
}

@MainActor
final class MacMediaSwiperComponent: NSObject, MacNativeComponent {
    let id: String
    let view: NSView

    private let pageHost: NSView
    private let dotsContainer: NSStackView
    private var dotsHorizontalConstraints: [NSLayoutConstraint] = []
    private var dotsVerticalConstraints: [NSLayoutConstraint] = []
    private let eventSink: ([String: Any]) -> Void
    private var pages: [Int: MacSwiperPage] = [:]
    private var config = MacMediaSwiperConfig()
    private var currentIndex = 0
    private var autoplayTimer: Timer?
    private var lastBoundsSize: CGSize = .zero
    private var transitionGeneration: UInt64 = 0

    init(id: String, initialProps: [String: Any], eventSink: @escaping ([String: Any]) -> Void) {
        self.id = id
        self.eventSink = eventSink

        let container = NSView()
        container.wantsLayer = true
        container.layer?.backgroundColor = NSColor.black.cgColor

        let host = NSView()
        host.wantsLayer = true
        host.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(host)

        let dots = NSStackView()
        dots.orientation = .horizontal
        dots.spacing = 8
        dots.alignment = .centerY
        dots.translatesAutoresizingMaskIntoConstraints = false
        dots.isHidden = true
        container.addSubview(dots)

        let dotsHorizontalConstraints = [
            dots.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -12),
            dots.centerXAnchor.constraint(equalTo: container.centerXAnchor),
        ]
        let dotsVerticalConstraints = [
            dots.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -12),
            dots.centerYAnchor.constraint(equalTo: container.centerYAnchor),
        ]

        NSLayoutConstraint.activate([
            host.topAnchor.constraint(equalTo: container.topAnchor),
            host.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            host.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            host.bottomAnchor.constraint(equalTo: container.bottomAnchor),
        ])
        NSLayoutConstraint.activate(dotsHorizontalConstraints)

        self.pageHost = host
        self.dotsContainer = dots
        self.dotsHorizontalConstraints = dotsHorizontalConstraints
        self.dotsVerticalConstraints = dotsVerticalConstraints
        self.view = container

        super.init()
        update(props: initialProps)
    }

    func mount(in host: NSView) {
        host.addSubview(view)
    }

    func update(props: [String: Any]) {
        let previousItems = config.items
        let previousIndex = currentIndex
        let priorItem = previousItems.indices.contains(previousIndex) ? previousItems[previousIndex] : nil
        let previousPeekPrevious = config.peekPrevious
        let previousPeekNext = config.peekNext
        let previousDirection = config.direction
        let next = MacMediaSwiperConfig.parse(props: props, previous: config)
        let itemsChanged = previousItems != next.items
        let layoutChanged = !itemsChanged && (
            previousPeekPrevious != next.peekPrevious ||
            previousPeekNext != next.peekNext ||
            previousDirection != next.direction
        )
        config = next

        if itemsChanged {
            currentIndex = resolveIndexForItemsChange(
                next: next,
                previousItems: previousItems,
                previousIndex: previousIndex,
                priorItem: priorItem
            )
            rebuildPages()
            relayoutPages(animated: false)
            updateDots()
            refreshVisiblePagesPlayback()
        } else {
            for (_, page) in pages {
                page.applyConfig(next)
            }
            if layoutChanged {
                relayoutPages(animated: false)
                updateDots()
            }
            if let controlled = next.index {
                let resolved = clampIndex(controlled, count: next.items.count)
                if resolved != currentIndex {
                    currentIndex = resolved
                    relayoutPages(animated: false)
                    updateDots()
                    refreshVisiblePagesPlayback()
                }
            }
        }

        scheduleAutoplay()
    }

    func setFrame(_ frame: CGRect) {
        if !view.frame.equalTo(frame) {
            view.frame = frame
        }
        view.layoutSubtreeIfNeeded()
        if lastBoundsSize != frame.size {
            lastBoundsSize = frame.size
            relayoutPages(animated: false)
        }
    }

    func focus() {
        // Symmetric with blur(): when the host regains focus, resume the current video
        // so it doesn't stay paused indefinitely after a window/page deactivation.
        if let page = pages[currentIndex] {
            page.onVisible()
        }
    }
    func blur() {
        // Mirror MacVideoComponent's blur: pause active video.
        if let page = pages[currentIndex] {
            page.onHidden()
        }
    }

    func handleCommand(name: String, params: [String: Any]?) {
        switch name {
        case "next":
            goBy(delta: 1, source: "api")
        case "previous":
            goBy(delta: -1, source: "api")
        case "goToIndex":
            guard let n = params?["index"] as? NSNumber else { return }
            let idx = n.intValue
            if idx < 0 || idx >= config.items.count { return }
            goTo(target: idx, source: "api", animated: config.animation != "none")
        default:
            break
        }
    }

    func unmount() {
        stopAutoplay()
        for (_, page) in pages { page.detach() }
        pages.removeAll()
        view.removeFromSuperview()
    }

    // MARK: - Pages

    private func rebuildPages() {
        for (_, page) in pages { page.detach() }
        pages.removeAll()
        for (index, item) in config.items.enumerated() {
            let page = MacSwiperPage(
                index: index,
                item: item,
                config: config,
                eventSink: { [weak self] event, detail in
                    self?.handlePageEvent(event: event, detail: detail)
                }
            )
            page.attach(to: pageHost)
            pages[index] = page
        }
    }

    private func relayoutPages(animated: Bool) {
        view.layoutSubtreeIfNeeded()
        let size = pageHost.bounds.size != .zero ? pageHost.bounds.size : view.bounds.size
        guard size.width > 0, size.height > 0 else { return }
        let horizontal = config.direction != "vertical"
        // Page stride along the active axis shrinks by `peekPrevious + peekNext` so
        // the current page leaves room for adjacent peeks. With zero peek the stride
        // matches the host dimension and behaves identically to v1.
        let mainSize = horizontal ? size.width : size.height
        let breadth = horizontal ? size.height : size.width
        let stride = max(1, mainSize - config.peekPrevious - config.peekNext)
        for (index, page) in pages {
            let offset = CGFloat(index - currentIndex) * stride
            let frame: CGRect
            if horizontal {
                // Centre current page within the host: shift right by peekPrevious so
                // page (i-1) peeks on the left when its frame moves into view.
                frame = CGRect(
                    x: config.peekPrevious + offset,
                    y: 0,
                    width: stride,
                    height: breadth
                )
            } else {
                // AppKit y grows upward; positive offset visually below current → negative y origin.
                frame = CGRect(
                    x: 0,
                    y: -(config.peekPrevious + offset),
                    width: breadth,
                    height: stride
                )
            }
            page.setFrame(frame, animated: animated)
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

    private func resolveInitialIndex(_ cfg: MacMediaSwiperConfig) -> Int {
        let raw = cfg.index ?? cfg.initialIndex
        return clampIndex(raw, count: cfg.items.count)
    }

    private func resolveIndexForItemsChange(
        next: MacMediaSwiperConfig,
        previousItems: [MacMediaSwiperItem],
        previousIndex: Int,
        priorItem: MacMediaSwiperItem?
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
        transitionGeneration &+= 1
        let generation = transitionGeneration
        emitChange(index: target, previous: previous, source: source)
        relayoutPages(animated: animated)

        if animated {
            let duration = max(0.05, Double(config.animationDuration) / 1000.0)
            DispatchQueue.main.asyncAfter(deadline: .now() + duration) { [weak self] in
                guard let self else { return }
                // Always emit transitionend for THIS transition so every change has a
                // matching transitionend, even if a re-entrant goTo bumped generation.
                self.emitTransitionEnd(index: target, previous: previous, source: source)
                if self.transitionGeneration == generation {
                    self.refreshVisiblePagesPlayback()
                }
            }
        } else {
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
        guard config.dotsEnabled, config.items.count > 1 else {
            dotsContainer.isHidden = true
            return
        }
        dotsContainer.isHidden = false
        let vertical = config.direction == "vertical"
        dotsContainer.orientation = vertical ? .vertical : .horizontal
        dotsContainer.alignment = vertical ? .centerX : .centerY
        NSLayoutConstraint.deactivate(vertical ? dotsHorizontalConstraints : dotsVerticalConstraints)
        NSLayoutConstraint.activate(vertical ? dotsVerticalConstraints : dotsHorizontalConstraints)

        let needed = config.items.count
        while dotsContainer.arrangedSubviews.count > needed {
            let last = dotsContainer.arrangedSubviews.last!
            dotsContainer.removeArrangedSubview(last)
            last.removeFromSuperview()
        }
        while dotsContainer.arrangedSubviews.count < needed {
            let dot = NSView()
            dot.wantsLayer = true
            dot.layer?.cornerRadius = 3
            dot.translatesAutoresizingMaskIntoConstraints = false
            NSLayoutConstraint.activate([
                dot.widthAnchor.constraint(equalToConstant: 6),
                dot.heightAnchor.constraint(equalToConstant: 6),
            ])
            dotsContainer.addArrangedSubview(dot)
        }
        for (i, dot) in dotsContainer.arrangedSubviews.enumerated() {
            dot.layer?.backgroundColor = (i == currentIndex
                ? config.dotsActiveColor : config.dotsColor).cgColor
        }
    }

    // MARK: - Autoplay

    private func scheduleAutoplay() {
        stopAutoplay()
        guard config.autoplay, config.items.count > 1 else { return }
        if !config.loop && currentIndex >= config.items.count - 1 { return }
        let interval = max(0.5, Double(config.interval) / 1000.0)
        autoplayTimer = Timer.scheduledTimer(withTimeInterval: interval, repeats: false) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.goBy(delta: 1, source: "autoplay")
            }
        }
    }

    private func stopAutoplay() {
        autoplayTimer?.invalidate()
        autoplayTimer = nil
    }
}

// MARK: - Config

private struct MacMediaSwiperItem: Equatable {
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

private struct MacMediaSwiperConfig: Equatable {
    var items: [MacMediaSwiperItem] = []
    var index: Int? = nil
    var initialIndex: Int = 0
    var loop: Bool = false
    var autoplay: Bool = false
    var interval: Int = 5000
    var animation: String = "slide"
    var animationDuration: Int = 300
    var direction: String = "horizontal"
    var rotate: Int = 0
    var objectFit: LxMediaObjectFit = .cover
    var controls: Bool = false
    var muted: Bool = true
    var dotsEnabled: Bool = false
    var dotsColor: NSColor = NSColor(white: 1, alpha: 0.4)
    var dotsActiveColor: NSColor = .white
    var swipeEnabled: Bool = true
    var peekPrevious: CGFloat = 0
    var peekNext: CGFloat = 0

    static func parse(props: [String: Any], previous: MacMediaSwiperConfig) -> MacMediaSwiperConfig {
        var next = previous

        if let raw = props["items"] as? [Any] {
            next.items = raw.enumerated().compactMap { index, entry in
                guard let map = entry as? [String: Any] else { return nil }
                guard let typeRaw = map["type"] as? String,
                      let kind = MacMediaSwiperItem.Kind(rawValue: typeRaw),
                      let src = (map["src"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines),
                      !src.isEmpty
                else { return nil }
                let id = (map["id"] as? String) ?? "\(typeRaw):\(src):\(index)"
                let poster = map["poster"] as? String
                let controls = map["controls"] as? Bool
                let muted = map["muted"] as? Bool
                return MacMediaSwiperItem(
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
        if let n = props["animationDuration"] as? NSNumber { next.animationDuration = max(0, n.intValue) }
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
            if let s = dict["color"] as? String, let c = NSColor.fromHexOrName(s) { next.dotsColor = c }
            if let s = dict["activeColor"] as? String, let c = NSColor.fromHexOrName(s) { next.dotsActiveColor = c }
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
private final class MacSwiperPage {
    let index: Int
    private let item: MacMediaSwiperItem
    private var config: MacMediaSwiperConfig
    private let eventSink: (_ event: String, _ detail: [String: Any]) -> Void

    private let container: NSView
    private var imageView: NSImageView?
    private var imageTask: URLSessionDataTask?
    private var imageRequestToken: UUID = UUID()
    private var player: MacLxMediaPlayer?
    private var clickRecognizer: NSClickGestureRecognizer?
    private var lastRenderKey: String?

    init(
        index: Int,
        item: MacMediaSwiperItem,
        config: MacMediaSwiperConfig,
        eventSink: @escaping (_ event: String, _ detail: [String: Any]) -> Void
    ) {
        self.index = index
        self.item = item
        self.config = config
        self.eventSink = eventSink
        let container = NSView()
        container.wantsLayer = true
        container.layer?.backgroundColor = NSColor.black.cgColor
        container.autoresizingMask = []
        self.container = container
    }

    func attach(to host: NSView) {
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

    func setFrame(_ frame: CGRect, animated: Bool) {
        let target = frame
        if animated {
            NSAnimationContext.runAnimationGroup { ctx in
                ctx.duration = max(0.05, Double(config.animationDuration) / 1000.0)
                ctx.allowsImplicitAnimation = true
                container.animator().frame = target
            }
        } else {
            container.frame = target
        }
        imageView?.frame = container.bounds
        applyImageRotation()
        player?.view.frame = container.bounds
        player?.layoutSubviews()
    }

    func applyConfig(_ config: MacMediaSwiperConfig) {
        self.config = config
        let renderKey = currentRenderKey()
        if renderKey == lastRenderKey {
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
        bind()
    }

    func onVisible() {
        if item.kind == .video {
            player?.handle(command: .play)
        }
    }

    func onHidden() {
        if item.kind == .video {
            player?.handle(command: .pause)
        }
    }

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
        imageTask?.cancel()
        imageTask = nil
        player?.detach()
        player = nil
        imageView = nil
        for sub in container.subviews { sub.removeFromSuperview() }
        lastRenderKey = renderKey

        switch item.kind {
        case .image: bindImage()
        case .video: bindVideo()
        }
        installClickGesture()
    }

    private func bindImage() {
        let image = NSImageView(frame: container.bounds)
        image.wantsLayer = true
        image.layer?.backgroundColor = NSColor.black.cgColor
        image.autoresizingMask = [.width, .height]
        container.addSubview(image)
        imageView = image
        applyImageContentMode()
        applyImageRotation()
        loadImage(into: image)
    }

    private func applyImageContentMode() {
        guard let imageView else { return }
        switch config.objectFit {
        case .cover:
            imageView.imageScaling = .scaleAxesIndependently
            imageView.layer?.contentsGravity = .resizeAspectFill
            imageView.layer?.masksToBounds = true
        case .contain, .fit:
            imageView.imageScaling = .scaleProportionallyUpOrDown
        case .fill:
            imageView.imageScaling = .scaleAxesIndependently
        }
    }

    private func applyImageRotation() {
        guard let imageView else { return }
        let degrees = CGFloat(config.rotate)
        imageView.frameRotation = degrees
    }

    private func bindVideo() {
        let player = MacLxMediaPlayer(eventSink: { [weak self] payload in
            self?.handleMediaPayload(payload)
        })
        let v = player.view
        v.frame = container.bounds
        v.autoresizingMask = [.width, .height]
        container.addSubview(v)
        var cfg = LxMediaPlayerConfig()
        if let url = MacMediaSwiperURL.resolve(item.src) {
            cfg.src = url
        }
        if let posterStr = item.poster, let posterURL = MacMediaSwiperURL.resolve(posterStr) {
            cfg.poster = posterURL
        }
        cfg.controls = item.controls ?? config.controls
        cfg.muted = item.muted ?? config.muted
        cfg.loop = false
        cfg.autoplay = false
        cfg.objectFit = config.objectFit
        cfg.rotateDegrees = config.rotate
        cfg.progressBar = (item.controls ?? config.controls)
        player.update(config: cfg)
        self.player = player
    }

    private func installClickGesture() {
        if let existing = clickRecognizer {
            container.removeGestureRecognizer(existing)
            clickRecognizer = nil
        }
        if item.kind == .video && (item.controls ?? config.controls) {
            return
        }
        let click = NSClickGestureRecognizer(target: self, action: #selector(handleClick))
        container.addGestureRecognizer(click)
        clickRecognizer = click
    }

    @objc private func handleClick() {
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
                "code": MacMediaSwiperError.normalize(code),
                "message": message,
            ])
        default:
            break
        }
    }

    private func loadImage(into target: NSImageView) {
        let token = UUID()
        imageRequestToken = token
        guard let url = MacMediaSwiperURL.resolve(item.src) else {
            emitError(code: "not_found", message: "image source not found")
            return
        }
        if url.isFileURL {
            let path = url.path
            if !FileManager.default.fileExists(atPath: path) {
                emitError(code: "not_found", message: "image file does not exist")
                return
            }
            DispatchQueue.global(qos: .userInitiated).async { [weak self] in
                let image = NSImage(contentsOfFile: path)
                Task { @MainActor [weak self] in
                    guard let self, self.imageRequestToken == token else { return }
                    if let image {
                        target.image = image
                    } else {
                        self.emitError(code: "decode", message: "image decode failed")
                    }
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
                guard let data, let image = NSImage(data: data) else {
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
            "code": MacMediaSwiperError.normalize(code),
            "message": message,
        ])
    }
}

// MARK: - Helpers

private enum MacMediaSwiperURL {
    @MainActor
    static func resolve(_ src: String) -> URL? {
        let trimmed = src.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { return nil }
        if trimmed.hasPrefix("http://") || trimmed.hasPrefix("https://") {
            return URL(string: trimmed)
        }
        let current = getCurrentLxApp()
        let appId = current.appid.toString()
        let resolved = resolveLxUri(appId, trimmed)?.toString() ?? trimmed
        if let url = URL(string: resolved), url.scheme != nil {
            return url
        }
        if resolved.hasPrefix("/") {
            return URL(fileURLWithPath: resolved)
        }
        return nil
    }
}

private enum MacMediaSwiperError {
    static let known: Set<String> = [
        "not_found", "network", "decode", "unsupported_format", "permission_denied", "unknown",
    ]
    static func normalize(_ code: String) -> String {
        let lower = code.lowercased()
        return known.contains(lower) ? lower : "unknown"
    }
}

private extension NSColor {
    static func fromHexOrName(_ raw: String) -> NSColor? {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { return nil }
        if trimmed.hasPrefix("#") {
            return colorFromHex(String(trimmed.dropFirst()))
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

    private static func colorFromHex(_ hex: String) -> NSColor? {
        var value: UInt64 = 0
        let scanner = Scanner(string: hex)
        guard scanner.scanHexInt64(&value) else { return nil }
        switch hex.count {
        case 6:
            let r = CGFloat((value & 0xFF0000) >> 16) / 255
            let g = CGFloat((value & 0x00FF00) >> 8) / 255
            let b = CGFloat(value & 0x0000FF) / 255
            return NSColor(srgbRed: r, green: g, blue: b, alpha: 1)
        case 8:
            // CSS-compatible RGBA ordering (#RRGGBBAA).
            let r = CGFloat((value & 0xFF000000) >> 24) / 255
            let g = CGFloat((value & 0x00FF0000) >> 16) / 255
            let b = CGFloat((value & 0x0000FF00) >> 8) / 255
            let a = CGFloat(value & 0x000000FF) / 255
            return NSColor(srgbRed: r, green: g, blue: b, alpha: a)
        default:
            return nil
        }
    }
}

#endif
