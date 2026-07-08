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
    private var playlist: [URL] = []
    private var playlistIndex = 0
    private var displayRotationDegrees: Int = 0

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
    private let settingsButton = NSButton()
    private var settingsPanel: NSView?
    private var settingsDismissOverlay: NSView?
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
        // Track Auto Layout resizes of the bar; layoutSubviews() is only
        // called on explicit setFrame/fullscreen, so the layer must follow
        // the superlayer on its own or it keeps a stale width.
        gradient.autoresizingMask = [.layerWidthSizable, .layerHeightSizable]
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

        configureButton(settingsButton, symbolName: "gearshape.fill", action: #selector(showSettingsMenu))
        settingsButton.isHidden = true  // shown only when speeds/qualities are configured
        bottomBar.addSubview(settingsButton)
        settingsButton.translatesAutoresizingMaskIntoConstraints = false

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

            settingsButton.leadingAnchor.constraint(equalTo: volumeSlider.trailingAnchor, constant: 8),
            settingsButton.centerYAnchor.constraint(equalTo: bottomBar.centerYAnchor),
            settingsButton.widthAnchor.constraint(equalToConstant: 28),
            settingsButton.heightAnchor.constraint(equalToConstant: 28),

            fullscreenButton.leadingAnchor.constraint(equalTo: settingsButton.trailingAnchor, constant: 8),
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
            self.advancePlaylist(reason: "ended")
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

        applyDisplayRotationTransform()
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

    // MARK: - Settings menu (speed / quality)

    private func refreshSettingsButtonVisibility() {
        // Only worth a menu when there's an actual choice to make.
        let hasSettings = core.playbackRates.count > 1 || core.qualities.count > 1
        settingsButton.isHidden = !hasSettings
    }

    @objc private func showSettingsMenu() {
        if settingsPanel != nil {
            dismissSettingsPanel()
            return
        }

        let panel = buildSettingsPanel()

        // Click-outside-to-dismiss overlay, kept inside the player view (no separate window).
        let overlay = NSView(frame: view.bounds)
        overlay.autoresizingMask = [.width, .height]
        overlay.addGestureRecognizer(NSClickGestureRecognizer(target: self, action: #selector(dismissSettingsPanel)))
        view.addSubview(overlay)
        view.addSubview(panel)
        settingsDismissOverlay = overlay
        settingsPanel = panel

        // Position above the gear, clamped to the player bounds (non-flipped: y grows up).
        let gear = settingsButton.convert(settingsButton.bounds, to: view)
        let w = panel.frame.width
        let h = panel.frame.height
        let x = min(max(gear.midX - w / 2, 8), max(8, view.bounds.width - w - 8))
        let y = min(max(gear.maxY + 8, 8), max(8, view.bounds.height - h - 8))
        panel.setFrameOrigin(NSPoint(x: x, y: y))

        controlsHideWorkItem?.cancel()  // keep the bar up while the panel is open
    }

    @objc private func dismissSettingsPanel() {
        settingsPanel?.removeFromSuperview()
        settingsDismissOverlay?.removeFromSuperview()
        settingsPanel = nil
        settingsDismissOverlay = nil
        scheduleHideControls()
    }

    private func buildSettingsPanel() -> NSView {
        let rowWidth: CGFloat = 184
        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 2
        stack.edgeInsets = NSEdgeInsets(top: 8, left: 8, bottom: 8, right: 8)
        stack.translatesAutoresizingMaskIntoConstraints = false

        // Quality section first (matches iOS).
        let qualities = core.qualities
        if qualities.count > 1 {
            stack.addArrangedSubview(makeSettingsHeader(L10n.string("lx_video_quality")))
            let active = core.activeQuality
            for quality in qualities {
                let label = quality.label
                stack.addArrangedSubview(makeSettingsRow(title: label, selected: label == active, width: rowWidth) { [weak self] in
                    self?.core.switchQuality(to: label)
                    self?.dismissSettingsPanel()
                })
            }
        }

        // Speed section.
        let rates = core.playbackRates
        if rates.count > 1 {
            if qualities.count > 1 { stack.addArrangedSubview(makeSettingsSeparator(width: rowWidth)) }
            stack.addArrangedSubview(makeSettingsHeader(L10n.string("lx_video_speed")))
            let active = core.activePlaybackRate
            for rate in rates {
                let title = "\(String(format: "%g", rate))x"
                stack.addArrangedSubview(makeSettingsRow(title: title, selected: abs(rate - active) < 0.001, width: rowWidth) { [weak self] in
                    self?.core.setPlaybackRate(rate)
                    self?.dismissSettingsPanel()
                })
            }
        }

        let panel = NSView()
        panel.wantsLayer = true
        panel.appearance = NSAppearance(named: .darkAqua)  // light text on the dark panel
        panel.layer?.backgroundColor = NSColor(calibratedWhite: 0.16, alpha: 0.98).cgColor
        panel.layer?.cornerRadius = 10
        panel.layer?.borderWidth = 0.5
        panel.layer?.borderColor = NSColor.white.withAlphaComponent(0.15).cgColor
        panel.layer?.shadowColor = NSColor.black.cgColor
        panel.layer?.shadowOpacity = 0.4
        panel.layer?.shadowRadius = 12
        panel.layer?.shadowOffset = NSSize(width: 0, height: -4)
        panel.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: panel.topAnchor),
            stack.bottomAnchor.constraint(equalTo: panel.bottomAnchor),
            stack.leadingAnchor.constraint(equalTo: panel.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: panel.trailingAnchor),
        ])
        panel.layoutSubtreeIfNeeded()
        panel.frame = NSRect(origin: .zero, size: panel.fittingSize)
        return panel
    }

    private func makeSettingsHeader(_ text: String) -> NSView {
        let label = NSTextField(labelWithString: text)
        label.font = NSFont.systemFont(ofSize: 11, weight: .semibold)
        label.textColor = .secondaryLabelColor
        label.translatesAutoresizingMaskIntoConstraints = false
        let wrap = NSView()
        wrap.addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: wrap.leadingAnchor, constant: 6),
            label.trailingAnchor.constraint(lessThanOrEqualTo: wrap.trailingAnchor, constant: -6),
            label.topAnchor.constraint(equalTo: wrap.topAnchor, constant: 4),
            label.bottomAnchor.constraint(equalTo: wrap.bottomAnchor, constant: -2),
        ])
        return wrap
    }

    private func makeSettingsSeparator(width: CGFloat) -> NSView {
        let line = NSBox()
        line.boxType = .separator
        line.translatesAutoresizingMaskIntoConstraints = false
        line.widthAnchor.constraint(equalToConstant: width).isActive = true
        return line
    }

    private func makeSettingsRow(title: String, selected: Bool, width: CGFloat, onClick: @escaping () -> Void) -> NSView {
        let row = SettingsRowButton(title: title, selected: selected, onClick: onClick)
        row.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            row.widthAnchor.constraint(equalToConstant: width),
            row.heightAnchor.constraint(equalToConstant: 26),
        ])
        return row
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
            core.setLoop(loop && playlist.count <= 1)
        }

        if let controls = config.controls {
            controlsEnabled = controls
            if !controls { hideControls() }
        }

        if let progressBar = config.progressBar {
            showProgressBar = progressBar
            progressSlider.isHidden = !progressBar
        }

        let clearProps = config.clearProps ?? []

        if clearProps.contains("objectFit") {
            let gravity: AVLayerVideoGravity = .resizeAspectFill
            core.setVideoGravity(gravity)
            playerLayerView.playerLayer.videoGravity = gravity
            applyDisplayRotationTransform()
        } else if let objectFit = config.objectFit {
            let gravity: AVLayerVideoGravity
            switch objectFit {
            case .cover: gravity = .resizeAspectFill
            case .contain, .fit: gravity = .resizeAspect
            case .fill: gravity = .resize
            }
            core.setVideoGravity(gravity)
            playerLayerView.playerLayer.videoGravity = gravity
            applyDisplayRotationTransform()
        }

        if clearProps.contains("rotate") {
            setDisplayRotationDegrees(nil)
        } else if let rotateDegrees = config.rotateDegrees {
            setDisplayRotationDegrees(rotateDegrees)
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

        if let speeds = config.speeds {
            core.setPlaybackRates(speeds)
        }

        refreshSettingsButtonVisibility()

        if let duration = config.duration, duration > 0 {
            core.setExternalDurationSeconds(duration)
        }

        if let poster = config.poster {
            if poster != posterURL {
                loadPoster(url: poster)
            }
        }

        var videoURL: URL?
        if let nextPlaylist = config.playlist, !nextPlaylist.isEmpty {
            applyPlaylist(nextPlaylist)
        } else if let source = config.source {
            playlist = []
            playlistIndex = 0
            core.setLoop(loopEnabled)
            switch source {
            case .url(let url): videoURL = url
            case .file(let path): videoURL = URL(fileURLWithPath: path)
            }
        } else if let src = config.src {
            playlist = []
            playlistIndex = 0
            core.setLoop(loopEnabled)
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

    private func applyPlaylist(_ urls: [URL]) {
        guard urls.count > 1 else {
            playlist = []
            playlistIndex = 0
            core.setLoop(loopEnabled)
            if let url = urls.first {
                loadPlaylistURL(url, autoplay: false)
            }
            return
        }
        if urls == playlist { return }
        playlist = urls
        playlistIndex = 0
        core.setLoop(false)
        loadPlaylistURL(urls[0], autoplay: false)
    }

    private func goToPlaylistIndex(_ target: Int, reason: String) {
        guard playlist.count > 1 else { return }
        let resolved: Int
        if loopEnabled {
            let n = playlist.count
            resolved = ((target % n) + n) % n
        } else {
            resolved = Swift.max(0, Swift.min(target, playlist.count - 1))
        }
        if resolved == playlistIndex { return }
        playlistIndex = resolved
        let url = playlist[resolved]
        rawEventSink(LxMediaEvent.playlistChange(index: resolved, url: url.absoluteString, reason: reason).rawPayload)
        loadPlaylistURL(url, autoplay: true)
    }

    private func advancePlaylist(reason: String) {
        guard playlist.count > 1 else { return }
        let isLast = playlistIndex >= playlist.count - 1
        if isLast && !loopEnabled {
            rawEventSink(LxMediaEvent.playlistEnd(index: playlistIndex, url: playlist[playlistIndex].absoluteString).rawPayload)
            return
        }
        goToPlaylistIndex(playlistIndex + 1, reason: reason)
    }

    private func loadPlaylistURL(_ url: URL, autoplay: Bool) {
        guard url != currentLoadingURL else { return }
        currentLoadingURL = url
        hasEverStartedPlayback = false
        stoppedByUser = true
        loadingIndicator.startAnimation(nil)
        loadingIndicator.isHidden = false
        showPoster()
        core.loadVideo(url: url)
        playerLayerView.playerLayer.player = core.player
        if autoplay {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in
                self?.core.play()
            }
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
        case .playlistNext:
            goToPlaylistIndex(playlistIndex + 1, reason: "manual")
        case .playlistPrevious:
            goToPlaylistIndex(playlistIndex - 1, reason: "manual")
        case .playlistGoToIndex(let target):
            goToPlaylistIndex(target, reason: "manual")
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

    private func setDisplayRotationDegrees(_ degrees: Int?) {
        let normalized = degrees.map(normalizeRotation)
        if let normalized,
           normalized != 0 && normalized != 90 && normalized != 180 && normalized != 270 {
            return
        }
        displayRotationDegrees = normalized ?? 0
        applyDisplayRotationTransform()
    }

    private func normalizeRotation(_ rotation: Int) -> Int {
        var normalized = rotation % 360
        if normalized < 0 {
            normalized += 360
        }
        return normalized
    }

    private func rotationScale(for degrees: Int) -> (x: CGFloat, y: CGFloat) {
        guard degrees == 90 || degrees == 270 else {
            return (1, 1)
        }
        let width = view.bounds.width
        let height = view.bounds.height
        guard width > 0, height > 0 else {
            return (1, 1)
        }
        let ratio1 = width / height
        let ratio2 = height / width
        switch playerLayerView.playerLayer.videoGravity {
        case .resizeAspectFill:
            let scale = max(ratio1, ratio2)
            return (scale, scale)
        case .resize:
            return (ratio1, ratio2)
        default:
            let scale = min(ratio1, ratio2)
            return (scale, scale)
        }
    }

    private func applyDisplayRotationTransform() {
        let angle = CGFloat(displayRotationDegrees) * (.pi / 180)
        let scale = rotationScale(for: displayRotationDegrees)
        let transform = CGAffineTransform(rotationAngle: angle).scaledBy(x: scale.x, y: scale.y)
        playerLayerView.layer?.setAffineTransform(transform)
        posterImageView.layer?.setAffineTransform(transform)
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

/// A full-width row in the settings panel: title + trailing checkmark, with hover highlight.
private final class SettingsRowButton: NSView {
    private let onClick: () -> Void
    private let titleLabel = NSTextField(labelWithString: "")
    private let checkmark = NSImageView()
    private var tracking: NSTrackingArea?

    init(title: String, selected: Bool, onClick: @escaping () -> Void) {
        self.onClick = onClick
        super.init(frame: .zero)
        wantsLayer = true
        layer?.cornerRadius = 5

        if selected {
            checkmark.image = NSImage(systemSymbolName: "checkmark", accessibilityDescription: nil)
        }
        checkmark.contentTintColor = .controlAccentColor
        checkmark.translatesAutoresizingMaskIntoConstraints = false
        addSubview(checkmark)

        titleLabel.stringValue = title
        titleLabel.font = NSFont.systemFont(ofSize: 12)
        titleLabel.textColor = .labelColor
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        addSubview(titleLabel)

        NSLayoutConstraint.activate([
            titleLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            titleLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            checkmark.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
            checkmark.centerYAnchor.constraint(equalTo: centerYAnchor),
            checkmark.widthAnchor.constraint(equalToConstant: 12),
            checkmark.heightAnchor.constraint(equalToConstant: 12),
        ])
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let existing = tracking { removeTrackingArea(existing) }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect],
            owner: self
        )
        addTrackingArea(area)
        tracking = area
    }

    override func mouseEntered(with event: NSEvent) {
        layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.1).cgColor
    }

    override func mouseExited(with event: NSEvent) {
        layer?.backgroundColor = NSColor.clear.cgColor
    }

    override func mouseUp(with event: NSEvent) {
        if bounds.contains(convert(event.locationInWindow, from: nil)) {
            onClick()
        }
    }
}

#endif
