#if os(macOS)
import Foundation
import AppKit
import AVFoundation

private final class PlayerLayerView: NSView {
    override func makeBackingLayer() -> CALayer {
        return AVPlayerLayer()
    }

    override var wantsUpdateLayer: Bool { true }

    var playerLayer: AVPlayerLayer {
        return layer as! AVPlayerLayer
    }

    init() {
        super.init(frame: .zero)
        wantsLayer = true
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
}

@MainActor
final class MacLxMediaPlayer: NSObject {
    let view: NSView
    private let container: MacPlayerContainerView

    private let core = LxMediaPlayerCore()
    private let playerLayerView = PlayerLayerView()
    private let rawEventSink: ([String: Any]) -> Void

    private var controlsEnabled = true
    private var showProgressBar = true
    private var loopEnabled = false

    private var controlsVisible = false
    private var controlsHideWorkItem: DispatchWorkItem?
    private var isFullscreen = false
    private var fullscreenWindow: NSWindow?
    nonisolated(unsafe) private var escMonitor: Any?
    private weak var originalSuperview: NSView?
    private var originalFrame: CGRect = .zero

    private var posterURL: URL?
    private var posterTask: Task<Void, Never>?
    private var hasEverStartedPlayback = false

    private var currentLoadingURL: URL?
    private var stoppedByUser = true

    private let overlayView = NSView()
    private let playAreaView = NSView()
    private let bottomBar = NSView()
    private let playPauseButton = NSButton()
    private let fullscreenButton = NSButton()
    private let timeLabel = NSTextField(labelWithString: "0:00 / 0:00")
    private let progressSlider = ThinSlider()
    private let volumeSlider = ThinSlider()
    private let volumeButton = NSButton()
    private let loadingIndicator = NSProgressIndicator()
    private let posterImageView = NSImageView()
    private var trackingArea: NSTrackingArea?

    init(eventSink: @escaping ([String: Any]) -> Void) {
        let c = MacPlayerContainerView(frame: .zero)
        self.container = c
        self.view = c
        self.rawEventSink = eventSink
        super.init()

        setupView()
        setupControls()
        bindCore()

        NotificationCenter.default.addObserver(
            self,
            selector: #selector(appWillResignActive),
            name: NSApplication.willResignActiveNotification,
            object: nil
        )
    }

    deinit {
        if let monitor = escMonitor {
            NSEvent.removeMonitor(monitor)
        }
        NotificationCenter.default.removeObserver(self)
    }

    // MARK: - Setup

    private func setupView() {
        view.wantsLayer = true
        view.layer?.backgroundColor = NSColor.black.cgColor

        playerLayerView.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(playerLayerView)
        NSLayoutConstraint.activate([
            playerLayerView.topAnchor.constraint(equalTo: view.topAnchor),
            playerLayerView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            playerLayerView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            playerLayerView.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])
        playerLayerView.playerLayer.videoGravity = core.playerLayer.videoGravity

        posterImageView.imageScaling = .scaleProportionallyUpOrDown
        posterImageView.wantsLayer = true
        posterImageView.isHidden = true
        posterImageView.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(posterImageView)
        NSLayoutConstraint.activate([
            posterImageView.topAnchor.constraint(equalTo: view.topAnchor),
            posterImageView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            posterImageView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            posterImageView.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])

        loadingIndicator.style = .spinning
        loadingIndicator.controlSize = .regular
        loadingIndicator.translatesAutoresizingMaskIntoConstraints = false
        loadingIndicator.isHidden = true
        view.addSubview(loadingIndicator)
        NSLayoutConstraint.activate([
            loadingIndicator.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            loadingIndicator.centerYAnchor.constraint(equalTo: view.centerYAnchor)
        ])

        overlayView.wantsLayer = true
        overlayView.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(overlayView)
        NSLayoutConstraint.activate([
            overlayView.topAnchor.constraint(equalTo: view.topAnchor),
            overlayView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            overlayView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            overlayView.bottomAnchor.constraint(equalTo: view.bottomAnchor)
        ])
        overlayView.isHidden = true
        overlayView.alphaValue = 0
    }

    private func setupControls() {
        playAreaView.translatesAutoresizingMaskIntoConstraints = false
        overlayView.addSubview(playAreaView)

        bottomBar.wantsLayer = true
        bottomBar.translatesAutoresizingMaskIntoConstraints = false
        overlayView.addSubview(bottomBar)
        NSLayoutConstraint.activate([
            bottomBar.leadingAnchor.constraint(equalTo: overlayView.leadingAnchor),
            bottomBar.trailingAnchor.constraint(equalTo: overlayView.trailingAnchor),
            bottomBar.bottomAnchor.constraint(equalTo: overlayView.bottomAnchor),
            bottomBar.heightAnchor.constraint(equalToConstant: 48)
        ])

        NSLayoutConstraint.activate([
            playAreaView.topAnchor.constraint(equalTo: overlayView.topAnchor),
            playAreaView.leadingAnchor.constraint(equalTo: overlayView.leadingAnchor),
            playAreaView.trailingAnchor.constraint(equalTo: overlayView.trailingAnchor),
            playAreaView.bottomAnchor.constraint(equalTo: bottomBar.topAnchor)
        ])

        let gradient = CAGradientLayer()
        gradient.colors = [
            NSColor.clear.cgColor,
            NSColor.black.withAlphaComponent(0.6).cgColor
        ]
        gradient.locations = [0, 1]
        bottomBar.layer?.addSublayer(gradient)

        configureButton(playPauseButton, symbolName: "play.fill", action: #selector(togglePlayPause))
        bottomBar.addSubview(playPauseButton)
        playPauseButton.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            playPauseButton.leadingAnchor.constraint(equalTo: bottomBar.leadingAnchor, constant: 8),
            playPauseButton.centerYAnchor.constraint(equalTo: bottomBar.centerYAnchor),
            playPauseButton.widthAnchor.constraint(equalToConstant: 28),
            playPauseButton.heightAnchor.constraint(equalToConstant: 28)
        ])

        progressSlider.minValue = 0
        progressSlider.maxValue = 1
        progressSlider.doubleValue = 0
        progressSlider.target = self
        progressSlider.action = #selector(progressSliderChanged(_:))
        progressSlider.isContinuous = true
        progressSlider.translatesAutoresizingMaskIntoConstraints = false
        bottomBar.addSubview(progressSlider)

        timeLabel.font = NSFont.monospacedDigitSystemFont(ofSize: 11, weight: .regular)
        timeLabel.textColor = .white
        timeLabel.translatesAutoresizingMaskIntoConstraints = false
        timeLabel.setContentCompressionResistancePriority(.required, for: .horizontal)
        bottomBar.addSubview(timeLabel)

        configureButton(volumeButton, symbolName: "speaker.wave.2.fill", action: #selector(toggleMute))
        bottomBar.addSubview(volumeButton)
        volumeButton.translatesAutoresizingMaskIntoConstraints = false

        volumeSlider.minValue = 0
        volumeSlider.maxValue = 1
        volumeSlider.doubleValue = 1
        volumeSlider.target = self
        volumeSlider.action = #selector(volumeSliderChanged(_:))
        volumeSlider.isContinuous = true
        volumeSlider.translatesAutoresizingMaskIntoConstraints = false
        bottomBar.addSubview(volumeSlider)

        configureButton(fullscreenButton, symbolName: "arrow.up.left.and.arrow.down.right", action: #selector(toggleFullscreen))
        bottomBar.addSubview(fullscreenButton)
        fullscreenButton.translatesAutoresizingMaskIntoConstraints = false

        NSLayoutConstraint.activate([
            progressSlider.leadingAnchor.constraint(equalTo: playPauseButton.trailingAnchor, constant: 8),
            progressSlider.centerYAnchor.constraint(equalTo: bottomBar.centerYAnchor),

            timeLabel.leadingAnchor.constraint(equalTo: progressSlider.trailingAnchor, constant: 8),
            timeLabel.centerYAnchor.constraint(equalTo: bottomBar.centerYAnchor),

            volumeButton.leadingAnchor.constraint(equalTo: timeLabel.trailingAnchor, constant: 8),
            volumeButton.centerYAnchor.constraint(equalTo: bottomBar.centerYAnchor),
            volumeButton.widthAnchor.constraint(equalToConstant: 28),
            volumeButton.heightAnchor.constraint(equalToConstant: 28),

            volumeSlider.leadingAnchor.constraint(equalTo: volumeButton.trailingAnchor, constant: 4),
            volumeSlider.centerYAnchor.constraint(equalTo: bottomBar.centerYAnchor),
            volumeSlider.widthAnchor.constraint(equalToConstant: 60),

            fullscreenButton.leadingAnchor.constraint(equalTo: volumeSlider.trailingAnchor, constant: 8),
            fullscreenButton.trailingAnchor.constraint(equalTo: bottomBar.trailingAnchor, constant: -8),
            fullscreenButton.centerYAnchor.constraint(equalTo: bottomBar.centerYAnchor),
            fullscreenButton.widthAnchor.constraint(equalToConstant: 28),
            fullscreenButton.heightAnchor.constraint(equalToConstant: 28),

            progressSlider.trailingAnchor.constraint(equalTo: timeLabel.leadingAnchor, constant: -8)
        ])

        let clickGesture = NSClickGestureRecognizer(target: self, action: #selector(overlayClicked))
        playAreaView.addGestureRecognizer(clickGesture)
        let rightClickGesture = NSClickGestureRecognizer(target: self, action: #selector(overlaySecondaryClicked))
        rightClickGesture.buttonMask = 0x2
        playAreaView.addGestureRecognizer(rightClickGesture)
        let layerClickGesture = NSClickGestureRecognizer(target: self, action: #selector(overlayClicked))
        playerLayerView.addGestureRecognizer(layerClickGesture)
        let layerRightClickGesture = NSClickGestureRecognizer(target: self, action: #selector(overlaySecondaryClicked))
        layerRightClickGesture.buttonMask = 0x2
        playerLayerView.addGestureRecognizer(layerRightClickGesture)

        progressSlider.isHidden = !showProgressBar
    }

    private func configureButton(_ button: NSButton, symbolName: String, action: Selector) {
        button.bezelStyle = .regularSquare
        button.isBordered = false
        button.wantsLayer = true
        button.layer?.backgroundColor = NSColor.clear.cgColor
        if let image = NSImage(systemSymbolName: symbolName, accessibilityDescription: nil) {
            let config = NSImage.SymbolConfiguration(pointSize: 14, weight: .medium)
            button.image = image.withSymbolConfiguration(config)
        }
        button.contentTintColor = .white
        button.target = self
        button.action = action
    }

    private func setButtonSymbol(_ button: NSButton, name: String) {
        if let image = NSImage(systemSymbolName: name, accessibilityDescription: nil) {
            let config = NSImage.SymbolConfiguration(pointSize: 14, weight: .medium)
            button.image = image.withSymbolConfiguration(config)
        }
    }

    private func bindCore() {
        core.onEvent = { [weak self] event in
            guard let self else { return }
            if case .seeked(let time) = event,
               let duration = self.core.effectiveDurationSeconds(), duration > 0 {
                self.updateProgressUI(currentTime: time, duration: duration)
            }
            self.rawEventSink(event.rawPayload)
        }

        core.onPlaybackStateChange = { [weak self] isPlaying, isBuffering in
            guard let self else { return }
            if isPlaying {
                self.stoppedByUser = false
            }
            self.updatePlayPauseUI(isPlaying: isPlaying)
            if isBuffering && !self.stoppedByUser {
                self.loadingIndicator.startAnimation(nil)
                self.loadingIndicator.isHidden = false
            } else {
                self.loadingIndicator.stopAnimation(nil)
                self.loadingIndicator.isHidden = true
            }
        }

        core.onReadyToPlay = { [weak self] in
            guard let self else { return }
            self.loadingIndicator.stopAnimation(nil)
            self.loadingIndicator.isHidden = true
        }

        core.onEnded = { [weak self] in
            guard let self else { return }
            self.updatePlayPauseUI(isPlaying: false)
        }

        core.onTimeUpdate = { [weak self] currentTime, duration in
            guard let self else { return }
            if !self.hasEverStartedPlayback && currentTime > 0 {
                self.hasEverStartedPlayback = true
                self.stoppedByUser = false
                self.hidePoster()
            }
            self.updateProgressUI(currentTime: currentTime, duration: duration)
        }

        core.onError = { [weak self] _, _ in
            guard let self else { return }
            self.loadingIndicator.stopAnimation(nil)
            self.loadingIndicator.isHidden = true
        }
    }

    // MARK: - Layout / Tracking area (mouse hover)

    func layoutSubviews() {
        if let gradient = bottomBar.layer?.sublayers?.first(where: { $0 is CAGradientLayer }) {
            gradient.frame = bottomBar.bounds
        }

        updateTrackingArea()
    }

    private func updateTrackingArea() {
        if let existing = trackingArea {
            view.removeTrackingArea(existing)
        }
        let area = NSTrackingArea(
            rect: view.bounds,
            options: [.mouseEnteredAndExited, .mouseMoved, .activeAlways],
            owner: self,
            userInfo: nil
        )
        view.addTrackingArea(area)
        trackingArea = area
    }

    @objc(mouseEntered:) func mouseEntered(with event: NSEvent) {
        showControls()
    }

    @objc(mouseExited:) func mouseExited(with event: NSEvent) {
        scheduleHideControls()
    }

    @objc(mouseMoved:) func mouseMoved(with event: NSEvent) {
        showControls()
        scheduleHideControls()
    }

    // MARK: - Controls visibility

    private func showControls() {
        guard controlsEnabled else { return }
        controlsHideWorkItem?.cancel()
        if overlayView.isHidden {
            overlayView.alphaValue = 0
            overlayView.isHidden = false
        }
        controlsVisible = true
        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = 0.2
            overlayView.animator().alphaValue = 1
        }
    }

    private func hideControls() {
        controlsVisible = false
        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = 0.3
            overlayView.animator().alphaValue = 0
        } completionHandler: { [weak self] in
            Task { @MainActor [weak self] in
                guard let self else { return }
                if !self.controlsVisible {
                    self.overlayView.isHidden = true
                }
            }
        }
    }

    private func scheduleHideControls() {
        controlsHideWorkItem?.cancel()
        let item = DispatchWorkItem { [weak self] in
            self?.hideControls()
        }
        controlsHideWorkItem = item
        DispatchQueue.main.asyncAfter(deadline: .now() + 3, execute: item)
    }

    private func showControlsTemporarily() {
        showControls()
        scheduleHideControls()
    }

    // MARK: - Actions

    @objc private func togglePlayPause() {
        if core.isPlaying {
            core.pause()
        } else {
            core.play()
        }
        showControlsTemporarily()
    }

    @objc private func overlayClicked() {
        if controlsVisible {
            togglePlayPause()
        } else {
            showControlsTemporarily()
        }
    }

    @objc private func overlaySecondaryClicked() {
        showControlsTemporarily()
    }

    @objc private func progressSliderChanged(_ sender: NSSlider) {
        guard let duration = core.effectiveDurationSeconds() else { return }
        let target = sender.doubleValue * duration
        core.seek(to: target)
    }

    @objc private func volumeSliderChanged(_ sender: NSSlider) {
        core.setVolume(sender.doubleValue)
        updateVolumeUI(volume: sender.doubleValue, muted: false)
    }

    @objc private func toggleMute() {
        let isMuted = core.player?.isMuted ?? false
        core.setMuted(!isMuted)
        updateVolumeUI(volume: volumeSlider.doubleValue, muted: !isMuted)
    }

    @objc private func toggleFullscreen() {
        if isFullscreen {
            exitFullscreen()
        } else {
            enterFullscreen()
        }
    }

    @objc private func appWillResignActive() {
        if core.isPlaying {
            core.pause()
        }
    }

    // MARK: - UI Updates

    private func updatePlayPauseUI(isPlaying: Bool) {
        let name = isPlaying ? "pause.fill" : "play.fill"
        setButtonSymbol(playPauseButton, name: name)
    }

    private func updateProgressUI(currentTime: Double, duration: Double) {
        guard duration > 0 else { return }
        progressSlider.doubleValue = currentTime / duration
        timeLabel.stringValue = "\(formatTime(currentTime)) / \(formatTime(duration))"
    }

    private func updateVolumeUI(volume: Double, muted: Bool) {
        let name: String
        if muted || volume < 0.01 {
            name = "speaker.slash.fill"
        } else if volume < 0.5 {
            name = "speaker.wave.1.fill"
        } else {
            name = "speaker.wave.2.fill"
        }
        setButtonSymbol(volumeButton, name: name)
    }

    private func formatTime(_ seconds: Double) -> String {
        guard seconds.isFinite else { return "0:00" }
        let total = Int(seconds)
        let m = total / 60
        let s = total % 60
        return String(format: "%d:%02d", m, s)
    }

    // MARK: - Poster

    private func loadPoster(url: URL) {
        posterTask?.cancel()
        posterURL = url
        posterImageView.image = nil
        posterImageView.isHidden = true
        posterTask = Task { [weak self] in
            do {
                let (data, _) = try await URLSession.shared.data(from: url)
                guard !Task.isCancelled, let self else { return }
                guard self.posterURL == url else { return }
                if let image = NSImage(data: data) {
                    self.posterImageView.image = image
                    self.showPoster()
                }
            } catch {
                if (error as? URLError)?.code == .cancelled || error is CancellationError { return }
            }
        }
    }

    private func showPoster() {
        guard !hasEverStartedPlayback else { return }
        posterImageView.isHidden = (posterImageView.image == nil)
    }

    private func hidePoster() {
        posterImageView.isHidden = true
    }

    private func clearPoster() {
        posterTask?.cancel()
        posterTask = nil
        posterURL = nil
        posterImageView.image = nil
        posterImageView.isHidden = true
    }

    // MARK: - Fullscreen

    private func enterFullscreen() {
        guard !isFullscreen, let screen = view.window?.screen ?? NSScreen.main else { return }
        isFullscreen = true
        originalSuperview = view.superview
        originalFrame = view.frame

        let fsWindow = NSWindow(
            contentRect: screen.frame,
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        fsWindow.level = .statusBar + 1
        fsWindow.backgroundColor = .black
        fsWindow.acceptsMouseMovedEvents = true
        fsWindow.contentView = NSView(frame: screen.frame)
        fsWindow.contentView?.wantsLayer = true

        view.removeFromSuperview()
        view.frame = fsWindow.contentView!.bounds
        view.autoresizingMask = [.width, .height]
        fsWindow.contentView?.addSubview(view)

        fsWindow.makeKeyAndOrderFront(nil)
        fullscreenWindow = fsWindow

        layoutSubviews()
        showControlsTemporarily()
        setButtonSymbol(fullscreenButton, name: "arrow.down.right.and.arrow.up.left")
        core.onEvent?(.fullscreenChange(fullScreen: true, direction: ""))

        escMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            if event.keyCode == 53 { // ESC
                self?.exitFullscreen()
                return nil
            }
            return event
        }
    }

    private func exitFullscreen() {
        guard isFullscreen else { return }
        isFullscreen = false

        if let monitor = escMonitor {
            NSEvent.removeMonitor(monitor)
            escMonitor = nil
        }

        view.removeFromSuperview()
        view.frame = originalFrame
        view.autoresizingMask = []
        originalSuperview?.addSubview(view)

        fullscreenWindow?.orderOut(nil)
        fullscreenWindow = nil

        layoutSubviews()
        setButtonSymbol(fullscreenButton, name: "arrow.up.left.and.arrow.down.right")
        core.onEvent?(.fullscreenChange(fullScreen: false, direction: ""))
    }

    // MARK: - Public API

    func update(config: LxMediaPlayerConfig) {
        if let loop = config.loop {
            loopEnabled = loop
            core.setLoop(loop)
        }

        if let controls = config.controls {
            controlsEnabled = controls
            if !controls { hideControls() }
        }

        if let progressBar = config.progressBar {
            showProgressBar = progressBar
            progressSlider.isHidden = !progressBar
        }

        if let objectFit = config.objectFit {
            let gravity: AVLayerVideoGravity
            switch objectFit {
            case .cover: gravity = .resizeAspectFill
            case .contain, .fit: gravity = .resizeAspect
            case .fill: gravity = .resize
            }
            core.setVideoGravity(gravity)
            playerLayerView.playerLayer.videoGravity = gravity
        }

        if let volume = config.volume {
            core.setVolume(volume)
            volumeSlider.doubleValue = volume
        }

        if let muted = config.muted {
            core.setMuted(muted)
            updateVolumeUI(volume: volumeSlider.doubleValue, muted: muted)
        }

        if let qualities = config.qualities {
            core.setQualities(qualities)
        }

        if let duration = config.duration, duration > 0 {
            core.setExternalDurationSeconds(duration)
        }

        if let poster = config.poster {
            if poster != posterURL {
                loadPoster(url: poster)
            }
        }

        var videoURL: URL?
        if let source = config.source {
            switch source {
            case .url(let url): videoURL = url
            case .file(let path): videoURL = URL(fileURLWithPath: path)
            }
        } else if let src = config.src {
            videoURL = src
        }

        if let url = videoURL, url != currentLoadingURL {
            currentLoadingURL = url
            hasEverStartedPlayback = false
            stoppedByUser = true
            loadingIndicator.startAnimation(nil)
            loadingIndicator.isHidden = false
            showPoster()
            core.loadVideo(url: url)

            playerLayerView.playerLayer.player = core.player

            if config.autoplay == true {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in
                    self?.core.play()
                }
            }
        }

        if let showControlsOnInit = config.showControlsOnInit, showControlsOnInit {
            showControlsTemporarily()
        }
    }

    func handle(command: LxMediaCommand) {
        switch command {
        case .play:
            stoppedByUser = false
            core.handle(command: .play)
        case .pause:
            core.handle(command: .pause)
        case .seek(let targetSeconds):
            stoppedByUser = false
            if let duration = core.effectiveDurationSeconds(), duration > 0 {
                let clamped = Swift.min(Swift.max(0, targetSeconds), duration)
                updateProgressUI(currentTime: clamped, duration: duration)
            }
            core.handle(command: .seek(time: targetSeconds))
        case .enterFullscreen:
            enterFullscreen()
        case .exitFullscreen:
            exitFullscreen()
        case .stop:
            stoppedByUser = true
            core.handle(command: .stop)
            hasEverStartedPlayback = false
            showPoster()
            progressSlider.doubleValue = 0
            loadingIndicator.stopAnimation(nil)
            loadingIndicator.isHidden = true
            if let duration = core.effectiveDurationSeconds(), duration > 0 {
                updateProgressUI(currentTime: 0, duration: duration)
            } else {
                timeLabel.stringValue = "0:00 / 0:00"
            }
            updatePlayPauseUI(isPlaying: false)
        default:
            core.handle(command: command)
        }
        showControlsTemporarily()
    }

    func setExternalDurationSeconds(_ seconds: Double?) {
        core.setExternalDurationSeconds(seconds)
    }

    func currentVolume() -> Double {
        let value = core.player?.volume ?? Float(volumeSlider.doubleValue)
        return Double(value)
    }

    func isMuted() -> Bool {
        return core.player?.isMuted ?? false
    }

    func detach() {
        if isFullscreen { exitFullscreen() }
        core.cleanup()
        clearPoster()
        controlsHideWorkItem?.cancel()
        view.removeFromSuperview()
    }
}

private final class MacPlayerContainerView: NSView {
    override var isFlipped: Bool { true }
}

final class ThinSlider: NSSlider {
    override class var cellClass: AnyClass? {
        get { ThinSliderCell.self }
        set { super.cellClass = newValue }
    }
}

private final class ThinSliderCell: NSSliderCell {
    private let trackHeight: CGFloat = 4
    private let knobDiameter: CGFloat = 10

    override func barRect(flipped: Bool) -> NSRect {
        let bounds = controlView?.bounds ?? super.barRect(flipped: flipped)
        let horizontalInset = knobDiameter / 2
        let width = Swift.max(0, bounds.width - horizontalInset * 2)
        return NSRect(
            x: bounds.minX + horizontalInset,
            y: bounds.midY - trackHeight / 2,
            width: width,
            height: trackHeight
        )
    }

    override func knobRect(flipped: Bool) -> NSRect {
        let bar = barRect(flipped: flipped)
        let range = Swift.max(0.000_1, maxValue - minValue)
        let fraction = CGFloat((doubleValue - minValue) / range).clamped(to: 0...1)
        let usable = bar.width - knobDiameter
        let x = bar.origin.x + usable * fraction
        let y = bar.midY - knobDiameter / 2
        return NSRect(x: x, y: y, width: knobDiameter, height: knobDiameter)
    }

    override func drawBar(inside rect: NSRect, flipped: Bool) {
        let bar = barRect(flipped: flipped)
        let radius = bar.height / 2

        let bgPath = NSBezierPath(roundedRect: bar, xRadius: radius, yRadius: radius)
        NSColor.white.withAlphaComponent(0.3).setFill()
        bgPath.fill()

        let range = Swift.max(0.000_1, maxValue - minValue)
        let fraction = CGFloat((doubleValue - minValue) / range).clamped(to: 0...1)
        var playedRect = bar
        playedRect.size.width = bar.width * fraction
        let playedPath = NSBezierPath(roundedRect: playedRect, xRadius: radius, yRadius: radius)
        NSColor.white.setFill()
        playedPath.fill()
    }

    override func drawKnob(_ knobRect: NSRect) {
        let inset = knobRect.insetBy(dx: 1, dy: 1)
        let path = NSBezierPath(ovalIn: inset)
        NSColor.white.setFill()
        path.fill()
    }
}

private extension CGFloat {
    func clamped(to range: ClosedRange<CGFloat>) -> CGFloat {
        Swift.min(Swift.max(self, range.lowerBound), range.upperBound)
    }
}

#endif
