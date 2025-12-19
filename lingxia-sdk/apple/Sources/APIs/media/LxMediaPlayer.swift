import Foundation
import UIKit
import AVFoundation
import OSLog
import Darwin

#if os(iOS)

public struct LxMediaPipe: @unchecked Sendable {
    public let id: String
    public let url: URL
    public let writeHandle: FileHandle?
    fileprivate let shouldUnlinkOnClose: Bool

    /// Create a named pipe owned by the player. Returns the write handle for native/Rust to push bytes.
    public static func make() throws -> LxMediaPipe {
        let id = UUID().uuidString
        let path = (NSTemporaryDirectory() as NSString).appendingPathComponent("lxpipe-\(id)")
        let mode: mode_t = 0o600
        if mkfifo(path, mode) != 0 && errno != EEXIST {
            throw NSError(domain: NSPOSIXErrorDomain, code: Int(errno), userInfo: [
                NSLocalizedDescriptionKey: "mkfifo failed with errno \(errno)"
            ])
        }
        let fd = Darwin.open(path, O_RDWR | O_NONBLOCK)
        guard fd >= 0 else {
            throw NSError(domain: NSPOSIXErrorDomain, code: Int(errno), userInfo: [
                NSLocalizedDescriptionKey: "open pipe failed with errno \(errno)"
            ])
        }
        let handle = FileHandle(fileDescriptor: fd, closeOnDealloc: true)
        return LxMediaPipe(id: id, url: URL(fileURLWithPath: path), writeHandle: handle, shouldUnlinkOnClose: true)
    }

    /// Wrap an external pipe path (e.g., provided by JS); no writer and no cleanup.
    public static func external(path: String) -> LxMediaPipe? {
        let url = URL(fileURLWithPath: path)
        return LxMediaPipe(id: url.lastPathComponent, url: url, writeHandle: nil, shouldUnlinkOnClose: false)
    }

    public func close() {
        try? writeHandle?.close()
        if shouldUnlinkOnClose {
            try? FileManager.default.removeItem(at: url)
        }
    }
}

public enum LxMediaSource {
    case url(URL)
    case file(path: String)
    case pipe(LxMediaPipe)

    var bridgeValue: [String: Any] {
        switch self {
        case .url(let url):
            return ["type": "url", "value": url.absoluteString]
        case .file(let path):
            return ["type": "file", "value": path]
        case .pipe(let pipe):
            return ["type": "pipe", "value": pipe.url.path]
        }
    }
}

public struct LxMediaQuality {
    public var label: String
    public var url: URL?

    public init(label: String, url: URL?) {
        self.label = label
        self.url = url
    }

    var bridgeValue: [String: Any] {
        var dict: [String: Any] = ["label": label]
        if let url {
            dict["url"] = url.absoluteString
        }
        return dict
    }
}

public enum LxMediaObjectFit: String {
    case cover
    case contain
    case fill
    case fit

    var bridgeValue: String {
        rawValue
    }
}

public struct LxMediaPlayerConfig {
    public var source: LxMediaSource?
    public var src: URL?
    public var poster: URL?
    public var autoplay: Bool?
    public var loop: Bool?
    public var muted: Bool?
    public var volume: Double?
    public var controls: Bool?  // Show or hide all playback controls (HTML5 standard)
    public var cornerRadius: Double?
    public var qualities: [LxMediaQuality]?  // First is default
    public var speeds: [Double]?             // First is default (playbackRates)
    public var showControlsOnInit: Bool?
    public var objectFit: LxMediaObjectFit?

    public init(
        source: LxMediaSource? = nil,
        src: URL? = nil,
        poster: URL? = nil,
        autoplay: Bool? = nil,
        loop: Bool? = nil,
        muted: Bool? = nil,
        volume: Double? = nil,
        controls: Bool? = nil,
        cornerRadius: Double? = nil,
        qualities: [LxMediaQuality]? = nil,
        speeds: [Double]? = nil,
        showControlsOnInit: Bool? = nil,
        objectFit: LxMediaObjectFit? = nil
    ) {
        self.source = source
        self.src = src
        self.poster = poster
        self.autoplay = autoplay
        self.loop = loop
        self.muted = muted
        self.volume = volume
        self.controls = controls
        self.cornerRadius = cornerRadius
        self.qualities = qualities
        self.speeds = speeds
        self.showControlsOnInit = showControlsOnInit
        self.objectFit = objectFit
    }

    var bridgeValue: [String: Any] {
        var dict: [String: Any] = [:]
        if let source {
            dict["source"] = source.bridgeValue
        }
        if let src { dict["src"] = src.absoluteString }
        if let poster { dict["poster"] = poster.absoluteString }
        if let autoplay { dict["autoplay"] = autoplay }
        if let loop { dict["loop"] = loop }
        if let muted { dict["muted"] = muted }
        if let volume { dict["volume"] = volume }
        if let controls { dict["controls"] = controls }
        if let cornerRadius { dict["cornerRadius"] = cornerRadius }
        if let qualities { dict["qualities"] = qualities.map { $0.bridgeValue } }
        if let speeds { dict["speeds"] = speeds }
        if let showControlsOnInit { dict["showControlsOnInit"] = showControlsOnInit }
        if let objectFit { dict["objectFit"] = objectFit.bridgeValue }
        return dict
    }
}

public enum LxMediaCommand {
    case play
    case pause
    case stop
    case seek(time: Double)
    case setVolume(Double)
    case setMuted(Bool)
    case setPlaybackRate(Double)
    case enterFullscreen
    case exitFullscreen
}

public enum LxMediaEvent {
    case play
    case pause
    case stop
    case ended
    case waiting
    case seeked(time: Double)
    case timeUpdate(currentTime: Double, duration: Double)
    case rateChange(rate: Double)
    case volumeChange(volume: Double)
    case fullscreenChange(fullScreen: Bool, direction: String)
    case loadedMetadata(width: Double, height: Double, duration: Double)
    case qualityChange(quality: String, url: String?)
    case error(code: String, message: String)
    case raw(name: String, data: [String: Any])

    var rawName: String {
        switch self {
        case .play: return "play"
        case .pause: return "pause"
        case .stop: return "stop"
        case .ended: return "ended"
        case .waiting: return "waiting"
        case .seeked: return "seeked"
        case .timeUpdate: return "timeupdate"
        case .rateChange: return "playbackratechange"
        case .volumeChange: return "volumechange"
        case .fullscreenChange: return "fullscreenchange"
        case .loadedMetadata: return "loadedmetadata"
        case .qualityChange: return "qualitychange"
        case .error: return "error"
        case .raw(let name, _): return name
        }
    }

    var rawData: [String: Any] {
        switch self {
        case .play, .pause, .stop, .ended, .waiting:
            return [:]
        case .seeked(let time):
            return ["time": time]
        case .timeUpdate(let currentTime, let duration):
            return ["currentTime": currentTime, "duration": duration]
        case .rateChange(let rate):
            return ["rate": rate]
        case .volumeChange(let volume):
            return ["volume": volume]
        case .fullscreenChange(let fullScreen, let direction):
            return ["fullScreen": fullScreen, "direction": direction]
        case .loadedMetadata(let width, let height, let duration):
            return ["width": width, "height": height, "duration": duration]
        case .qualityChange(let quality, let url):
            return ["quality": quality, "url": url ?? ""]
        case .error(let code, let message):
            return ["code": code, "message": message]
        case .raw(_, let data):
            return data
        }
    }

    var rawPayload: [String: Any] {
        return [
            "event": rawName,
            "detail": rawData
        ]
    }
}

// Lightweight media player with built-in native controls.
// Designed to be reused by SameLevel components and other media scenarios.
private final class PlayerContainerView: UIView {
    weak var overlay: UIView?

    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        // Allow popups (settings menu + scrim) to receive touches first
        for subview in subviews.reversed() where subview.tag == 9998 || subview.tag == 9999 {
            let localPoint = convert(point, to: subview)
            if let hit = subview.hitTest(localPoint, with: event) {
                return hit
            }
        }

        // Check overlay for tap catching and controls
        if let overlay = overlay,
           overlay.point(inside: convert(point, to: overlay), with: event) {
            let convertedPoint = convert(point, to: overlay)
            if let hit = overlay.hitTest(convertedPoint, with: event) {
                return hit
            }
        }

        return super.hitTest(point, with: event)
    }
}

/// Custom slider that uses pan gesture to bypass WKScrollView's gesture interference
private final class DebugSlider: UISlider {
    private var panGesture: UIPanGestureRecognizer!
    private var tapGesture: UITapGestureRecognizer!
    private var initialValue: Float = 0
    
    /// Callback for real-time scrubbing (called during pan with throttling)
    var onScrub: ((Float) -> Void)?
    /// Callback for final seek (called on pan end or tap)
    var onSeekComplete: ((Float) -> Void)?
    /// Callback for scrub start
    var onScrubStart: (() -> Void)?
    /// Callback for scrub end
    var onScrubEnd: (() -> Void)?
    
    private var lastScrubTime: TimeInterval = 0
    
    override init(frame: CGRect) {
        super.init(frame: frame)
        setupGestures()
    }
    
    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setupGestures()
    }
    
    private func setupGestures() {
        // Pan gesture for dragging
        panGesture = UIPanGestureRecognizer(target: self, action: #selector(handlePan(_:)))
        panGesture.cancelsTouchesInView = true
        panGesture.delaysTouchesBegan = false
        addGestureRecognizer(panGesture)
        
        // Tap gesture for jumping to position
        tapGesture = UITapGestureRecognizer(target: self, action: #selector(handleTap(_:)))
        tapGesture.require(toFail: panGesture)
        addGestureRecognizer(tapGesture)
    }
    
    @objc private func handleTap(_ gesture: UITapGestureRecognizer) {
        let location = gesture.location(in: self)
        let trackRect = self.trackRect(forBounds: bounds)
        
        // Calculate value based on tap position
        let percentage = Float((location.x - trackRect.minX) / trackRect.width)
        let newValue = min(max(percentage, 0), 1)
        value = newValue
        
        onScrubStart?()
        sendActions(for: .touchDown)
        sendActions(for: .valueChanged)
        onSeekComplete?(newValue)
        onScrubEnd?()
        sendActions(for: .touchUpInside)
    }
    
    @objc private func handlePan(_ gesture: UIPanGestureRecognizer) {
        let trackRect = self.trackRect(forBounds: bounds)
        
        switch gesture.state {
        case .began:
            initialValue = value
            onScrubStart?()
            sendActions(for: .touchDown)
            
        case .changed:
            let location = gesture.location(in: self)
            let percentage = Float((location.x - trackRect.minX) / trackRect.width)
            let newValue = min(max(percentage, 0), 1)
            
            if newValue != value {
                value = newValue
                sendActions(for: .valueChanged)
                
                // Throttled scrub callback (every 100ms)
                let now = CACurrentMediaTime()
                if now - lastScrubTime > 0.1 {
                    lastScrubTime = now
                    onScrub?(newValue)
                }
            }
            
        case .ended, .cancelled:
            onSeekComplete?(value)
            onScrubEnd?()
            sendActions(for: .touchUpInside)
            
        default:
            break
        }
    }
}

private final class TapOverlayView: UIView {
    weak var tapTarget: UIView?

    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        // First, try standard UIKit hitTest - this handles UISlider correctly
        let standardHit = super.hitTest(point, with: event)
        
        // If we found a control (button, slider, etc.), use it
        if let hit = standardHit, hit is UIControl {
            return hit
        }
        
        // If standard hitTest found something other than self, use it
        if let hit = standardHit, hit !== self {
            return hit
        }
        
        // Fallback to tap catcher for empty areas (to show/hide controls)
        if let target = tapTarget, bounds.contains(point) {
            return target
        }
        
        return standardHit
    }
}

@MainActor
public final class LxMediaPlayer: NSObject {
    public let view: UIView
    private let container: PlayerContainerView
    private let log = OSLog(subsystem: "LingXia", category: "Media")

    private let playerLayer = AVPlayerLayer()
    private var player: AVPlayer?
    private var timeObserver: Any?
    private var statusObserver: NSKeyValueObservation?
    private var timeControlStatusObserver: NSKeyValueObservation?
    private var videoOutput: AVPlayerItemVideoOutput?
    private var displayLink: CADisplayLink?
    private let rawEventSink: ([String: Any]) -> Void
    private let typedEventSink: ((LxMediaEvent) -> Void)?

    // Config
    private var controlsEnabled = true  // HTML5 standard: show/hide all controls
    private var showCloseButton = false // Only show in preview mode
    private var showFullscreenButton = true // Show fullscreen button
    private var loopEnabled = false // Loop playback when video ends
    private var videoGravity: AVLayerVideoGravity = .resizeAspectFill // Default to fill for SameLevel

    // State
    private var shouldShowControlsOnFirstPlay = false // Show controls on first play (for preview mode)
    private var currentLoadingURL: URL? // Track currently loading URL to avoid duplicate loads
    private var loadingSequence: UInt64 = 0 // Sequence number to identify stale callbacks

    // Quality and Speed
    private var availableQualities: [LxMediaQuality] = []
    private var currentQuality: String?
    private var availablePlaybackRates: [Double] = []
    private var currentPlaybackRate: Double = 1.0
    private var pendingRestoreAfterLoad: (time: CMTime, play: Bool)?

    // UI state
    private var controlsVisible = false
    private var controlsHideWorkItem: DispatchWorkItem?
    // Note: tapRecognizer removed - using tapCatcher button instead
    private var wasPlayingBeforeBackground = false
    private var isPausedByUser = false  // Track if user explicitly paused (vs buffering)
    private var firstFrameDisplayed = false
    private var waitingForFirstFrame = false
    private var desiredPlayWhenReady = false
    private var revealVideoWorkItem: DispatchWorkItem?
    private var forceRevealAttempts = 0
    private var pendingPlayEvent = false  // Delay play event until video actually starts
    private var lastTimeForPlayEvent: Double = -1  // Track time progression for play event

    // UI
    private let overlayView = TapOverlayView()
    private let topGradient = CAGradientLayer()
    private let bottomGradient = CAGradientLayer()
    private let topBar = UIView()
    private let bottomBar = UIView()
    private let playPauseButton = UIButton(type: .system)
    private let fullscreenButton = UIButton(type: .system)
    private let backButton = UIButton(type: .system)
    private let titleLabel = UILabel()
    private let timeLabel = UILabel() // Shows remaining or total time
    private let progressSlider = DebugSlider()
    private var isScrubbing = false
    private var wasPlayingBeforeScrub = false
    private var lastScrubTime: TimeInterval = 0
    private var isSeeking = false  // Track JS API seek in progress
    private var suppressWaitingUntil: TimeInterval = 0
    private let volumeSlider = UISlider()
    private let volumeButton = UIButton(type: .system)
    private let settingsButton = UIButton(type: .system)
    private let loadingIndicator = UIActivityIndicatorView(style: .large)
    private let tapCatcher = UIButton(type: .custom)
    private let posterImageView = UIImageView()

    // Fullscreen state
    private weak var originalSuperview: UIView?
    private var originalFrame: CGRect = .zero
    private var originalTransform: CGAffineTransform = .identity
    private var originalAutoresizingMask: UIView.AutoresizingMask = []
    private var isFullscreen = false
    private var isTransitioningFullscreen = false  // Flag to ignore updates during transition
    private var fullscreenWindow: UIWindow?
    private var fullscreenViewController: FullscreenPlayerViewController?
    private var activePipe: LxMediaPipe?

    // Poster state
    private var posterURL: URL?
    private var posterTask: Task<Void, Never>?
    private var lastTimeForPosterHide: Double = -1  // Track time for poster hiding (like HarmonyOS)
    private var pendingPosterHide = false  // Flag to delay poster hiding until time progresses

    public init(
        eventSink: @escaping ([String: Any]) -> Void,
        typedEventSink: ((LxMediaEvent) -> Void)? = nil
    ) {
        let container = PlayerContainerView(frame: .zero)
        self.container = container
        self.view = container
        self.rawEventSink = eventSink
        self.typedEventSink = typedEventSink
        super.init()

        // Ensure audio plays even in silent mode
        do {
            try AVAudioSession.sharedInstance().setCategory(.playback, mode: .moviePlayback, options: [.allowAirPlay, .allowBluetooth])
            try AVAudioSession.sharedInstance().setActive(true)
        } catch {
            os_log("MediaPlayer failed to set audio session: %{public}@", log: OSLog(subsystem: "LingXia", category: "Media"), type: .error, error.localizedDescription)
        }

        view.backgroundColor = .black
        view.clipsToBounds = true
        view.isUserInteractionEnabled = true
        view.isOpaque = false
        view.layer.zPosition = 2000

        playerLayer.videoGravity = videoGravity
        view.layer.addSublayer(playerLayer)

        // Setup poster image view (under playerLayer, above background)
        posterImageView.contentMode = .scaleAspectFill
        posterImageView.clipsToBounds = true
        posterImageView.backgroundColor = .black
        posterImageView.isHidden = true
        view.insertSubview(posterImageView, at: 0)

        setupOverlayUI()

        // Note: Removed tap gesture recognizer from root view - using tapCatcher button instead
        // This prevents gesture recognizer from interfering with UISlider drag
    }

    public convenience init(eventHandler: @escaping (LxMediaEvent) -> Void) {
        self.init(eventSink: { _ in }, typedEventSink: eventHandler)
    }

    deinit {
        // Safe cleanup even if called off the main actor
        if let pipe = activePipe, pipe.shouldUnlinkOnClose {
            pipe.close()
        }
    }

    // MARK: Public API

    public func attach(to host: UIView) {
        host.addSubview(view)
        host.bringSubviewToFront(view)
        view.isUserInteractionEnabled = true
        overlayView.isUserInteractionEnabled = true
        
        // Find parent WKScrollView and configure its pan gesture to not interfere with slider
        configureParentScrollViewGestures(from: host)
    }
    
    /// Configure parent scroll view's gestures to allow slider interaction
    private func configureParentScrollViewGestures(from view: UIView) {
        // Get slider's pan gesture
        guard let sliderPanGesture = progressSlider.gestureRecognizers?.first(where: { $0 is UIPanGestureRecognizer }) else {
            os_log("MediaPlayer: slider pan gesture not found", log: log, type: .error)
            return
        }
        
        var current: UIView? = view
        while let parent = current?.superview {
            // Check if this is a scroll view (WKScrollView, WKChildScrollView, or UIScrollView)
            if let scrollView = parent as? UIScrollView {
                // Make scroll view's pan gesture require slider's pan gesture to fail first
                scrollView.panGestureRecognizer.require(toFail: sliderPanGesture)
            }
            current = parent
        }
    }

    /// Configure whether to show close button (for preview scenarios)
    public func setShowCloseButton(_ show: Bool) {
        showCloseButton = show
        updateCloseButtonVisibility()
    }

    public func setShowFullscreenButton(_ show: Bool) {
        showFullscreenButton = show
        updateFullscreenButtonVisibility()
    }

    /// Mark the player as being in fullscreen mode (for external fullscreen management)
    /// This is used when the player is displayed in a fullscreen window/controller
    /// managed externally (e.g., MediaPreview)
    public func setFullscreenMode(_ fullscreen: Bool) {
        isFullscreen = fullscreen
        layoutOverlay()
    }

    public func setFrame(_ frame: CGRect) {
        // Ignore frame updates during fullscreen transition
        // The saved originalFrame will be restored after exit
        if isTransitioningFullscreen {
            os_log("MediaPlayer setFrame ignored during fullscreen transition", log: OSLog(subsystem: "LingXia", category: "Media"), type: .debug)
            return
        }

        if view.frame.equalTo(frame) {
            return
        }

        view.frame = frame
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        playerLayer.frame = view.bounds
        CATransaction.commit()
        layoutOverlay()
    }

    public func update(config: LxMediaPlayerConfig) {
        // Loop flag - only update if explicitly set in config
        if let loop = config.loop {
            loopEnabled = loop
        }

        // Poster
        if let poster = config.poster {
            loadPoster(urlString: poster.absoluteString)
        }

        // Source
        var nextPipe: LxMediaPipe?
        if let source = config.source {
            switch source {
            case .url(let url):
                loadVideo(url: url)
            case .file(let path):
                loadVideo(url: URL(fileURLWithPath: path))
            case .pipe(let pipe):
                nextPipe = pipe
                loadVideo(url: pipe.url)
            }
        } else if let src = config.src {
            loadVideo(url: src)
        }
        replaceActivePipe(with: nextPipe)

        // Always use .pause to receive end notification, handle loop in videoDidEnd
        player?.actionAtItemEnd = .pause

        // Playback flags
        if let autoplay = config.autoplay, autoplay {
            play()
        } else {
            updatePlayPauseUI(isPlaying: false)
        }

        if let muted = config.muted {
            player?.isMuted = muted
            // Update UI to reflect muted state
            if muted {
                updateVolumeIcon(volume: 0)
            } else if let vol = player?.volume {
                updateVolumeIcon(volume: vol)
            }
        }

        if let vol = config.volume {
            let volume = Float(vol)
            player?.volume = volume
            volumeSlider.value = volume
            updateVolumeIcon(volume: volume)
        }

        if let controls = config.controls {
            controlsEnabled = controls
            updateControlsVisibility()
        }

        if let radius = config.cornerRadius {
            let r = CGFloat(radius)
            view.layer.cornerRadius = r
            view.layer.masksToBounds = true
            playerLayer.cornerRadius = r
        }

        // Quality and Speed configuration
        if let qualities = config.qualities {
            availableQualities = qualities
            let labels = qualities.map { $0.label }
            if let existing = currentQuality, labels.contains(existing) {
                currentQuality = existing
            } else if let first = labels.first {
                currentQuality = first
            } else {
                currentQuality = nil
            }
        }

        if let speeds = config.speeds {
            availablePlaybackRates = speeds
            if let current = speeds.first {
                currentPlaybackRate = current
            } else {
                currentPlaybackRate = 1.0
            }
        }

        if let showControls = config.showControlsOnInit {
            shouldShowControlsOnFirstPlay = showControls
        }

        // Video display mode: "fill" (resizeAspectFill) or "fit" (resizeAspect)
        if let objectFit = config.objectFit {
            switch objectFit {
            case .cover, .fill:
                videoGravity = .resizeAspectFill
                posterImageView.contentMode = .scaleAspectFill
            case .contain, .fit:
                videoGravity = .resizeAspect
                posterImageView.contentMode = .scaleAspectFit
            }
            playerLayer.videoGravity = videoGravity
        }

        updateSettingsMenu()
    }

    public func handle(command: LxMediaCommand) {
        switch command {
        case .play: play()
        case .pause: pause()
        case .stop: stop()
        case .seek(let time): seek(to: time)
        case .setVolume(let volume):
            let vol = Float(volume)
            player?.volume = vol
            volumeSlider.value = vol
            updateVolumeIcon(volume: vol)
        case .setMuted(let muted):
            player?.isMuted = muted
            // Update UI to reflect muted state
            if muted {
                updateVolumeIcon(volume: 0)
            } else if let vol = player?.volume {
                updateVolumeIcon(volume: vol)
            }
        case .setPlaybackRate(let rate):
            setPlaybackRate(rate)
        case .enterFullscreen:
            enterFullscreen()
        case .exitFullscreen:
            exitFullscreen()
        }
    }

    public func detach() {
        stop()
        timeObserver.flatMap { player?.removeTimeObserver($0) }
        timeObserver = nil
        statusObserver?.invalidate()
        statusObserver = nil
        timeControlStatusObserver?.invalidate()
        timeControlStatusObserver = nil
        stopDisplayLink()
        videoOutput = nil
        posterTask?.cancel()
        posterTask = nil
        revealVideoWorkItem?.cancel()
        revealVideoWorkItem = nil
        NotificationCenter.default.removeObserver(self)
        player = nil
        view.removeFromSuperview()
        replaceActivePipe(with: nil)
    }

    // MARK: Player core

    private func loadPoster(urlString: String) {
        guard let url = URL(string: urlString) else { return }
        if posterURL == url {
            if posterImageView.image != nil || posterTask != nil {
                return
            }
        }
        posterURL = url

        // Cancel previous task
        posterTask?.cancel()

        // Download and display poster image
        posterTask = Task { @MainActor in
            defer { self.posterTask = nil }
            do {
                let (data, _) = try await URLSession.shared.data(from: url)
                guard !Task.isCancelled, let image = UIImage(data: data) else { return }
                posterImageView.image = image
                posterImageView.isHidden = false
                os_log("MediaPlayer loaded poster", log: OSLog(subsystem: "LingXia", category: "Media"), type: .info)
            } catch {
                if (error as? URLError)?.code == .cancelled || error is CancellationError { return }
                os_log("MediaPlayer failed to load poster: %{public}@", log: OSLog(subsystem: "LingXia", category: "Media"), type: .error, error.localizedDescription)
            }
        }
    }

    private func loadVideo(url: URL) {
        // Increment sequence number for this load operation
        loadingSequence &+= 1
        let currentSequence = loadingSequence
        currentLoadingURL = url

        os_log("MediaPlayer loadVideo seq=%llu url=%{public}@", log: log, type: .info, currentSequence, url.absoluteString)

        // Cancel any pending reveal work from previous loads
        revealVideoWorkItem?.cancel()
        revealVideoWorkItem = nil
        forceRevealAttempts = 0
        desiredPlayWhenReady = false

        // Clean up observers from previous loads
        timeObserver.flatMap { player?.removeTimeObserver($0) }
        timeObserver = nil
        statusObserver?.invalidate()
        statusObserver = nil
        timeControlStatusObserver?.invalidate()
        timeControlStatusObserver = nil

        // Keep overlay (controls) and loading indicator above everything
        view.bringSubviewToFront(overlayView)
        overlayView.bringSubviewToFront(loadingIndicator)
        loadingIndicator.startAnimating()

        // DON'T hide old video or show poster yet - keep current frame visible
        // This prevents black screen during switching
        // Only show poster if there's no current video playing
        if !firstFrameDisplayed && posterImageView.image != nil {
            posterImageView.isHidden = false
        }

        waitingForFirstFrame = true
        firstFrameDisplayed = false

        // Set playerLayer to opacity 0 to pre-render in background
        // This allows AVPlayer to render frames without showing them
        // Old video stays visible until crossfade
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        playerLayer.opacity = 0
        playerLayer.isHidden = false
        CATransaction.commit()

        let item = AVPlayerItem(url: url)
        // Preserve pitch when changing playback speed
        if #available(iOS 15.0, *) {
            item.audioTimePitchAlgorithm = .timeDomain
        } else {
            item.audioTimePitchAlgorithm = .lowQualityZeroLatency
        }
        item.preferredForwardBufferDuration = 2

        // Setup AVPlayerItemVideoOutput for reliable frame detection
        // This is the GOLD STANDARD for knowing when actual pixel data is available
        let pixelBufferAttributes: [String: Any] = [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA
        ]
        let output = AVPlayerItemVideoOutput(pixelBufferAttributes: pixelBufferAttributes)
        item.add(output)
        videoOutput = output

        let activePlayer: AVPlayer
        if let existing = player {
            existing.replaceCurrentItem(with: item)
            activePlayer = existing
        } else {
            activePlayer = AVPlayer(playerItem: item)
            player = activePlayer
        }

        // Always use .pause so we get end callback and can restart manually
        activePlayer.actionAtItemEnd = .pause
        activePlayer.rate = 0
        activePlayer.automaticallyWaitsToMinimizeStalling = true
        playerLayer.player = activePlayer
        os_log("MediaPlayer load url=%{public}@", log: log, type: .info, url.absoluteString)

        // Observe player item status - buffering ready
        statusObserver = item.observe(\.status, options: [.new]) { [weak self] item, _ in
            Task { @MainActor [weak self] in
                guard let self = self else { return }
                guard currentSequence == self.loadingSequence else {
                    os_log("MediaPlayer IGNORE stale statusObserver seq=%llu (current=%llu)", log: self.log, type: .debug, currentSequence, self.loadingSequence)
                    return
                }
                switch item.status {
                case .readyToPlay:
                    os_log("MediaPlayer item ready to play seq=%llu", log: self.log, type: .info, currentSequence)

                    // Send loadedmetadata event
                    let duration = item.duration.seconds
                    let size = item.presentationSize
                    if duration.isFinite && size.width > 0 {
                         self.send(.loadedMetadata(width: size.width, height: size.height, duration: duration))
                    }

                    if let pending = self.pendingRestoreAfterLoad {
                        self.pendingRestoreAfterLoad = nil
                        self.player?.seek(to: pending.time, toleranceBefore: .zero, toleranceAfter: .zero) { [weak self] _ in
                            guard pending.play else { return }
                            self?.play()
                        }
                    }

                    // Start frame detection now that item is ready
                    if self.waitingForFirstFrame {
                        self.startDisplayLink(sequence: currentSequence)
                        self.scheduleRevealTimeout(sequence: currentSequence)
                        // Lightweight fallback: trigger one manual check
                        // In case displayLink fails on some devices/iOS versions
                        self.checkPixelBufferAvailability(sequence: currentSequence)
                    }
                case .failed:
                    let msg = item.error?.localizedDescription ?? "unknown error"
                    os_log("MediaPlayer item failed seq=%llu error=%{public}@", log: self.log, type: .error, currentSequence, msg)
                    self.send(.error(code: "load_failed", message: msg))
                    self.stopDisplayLink()
                    self.revealVideoWorkItem?.cancel()
                    self.waitingForFirstFrame = false
                    self.firstFrameDisplayed = false
                    self.loadingIndicator.stopAnimating()
                    if let image = self.posterImageView.image, !image.size.equalTo(.zero) {
                        self.posterImageView.isHidden = false
                    }
                default:
                    break
                }
            }
        }
        
        // Observe player timeControlStatus for play/pause/buffering state changes
        timeControlStatusObserver = player?.observe(\.timeControlStatus, options: [.new, .old]) { [weak self] player, change in
            Task { @MainActor [weak self] in
                guard let self = self else { return }
                guard currentSequence == self.loadingSequence else { return }

                switch player.timeControlStatus {
                case .playing:
                    // Video is playing - stop loading indicator and send play event
                    self.loadingIndicator.stopAnimating()
                    self.updatePlayPauseUI(isPlaying: true)
                    self.suppressWaitingUntil = 0
                    self.send(.play)

                case .paused:
                    // Don't send pause event here - it's sent explicitly in pause() method
                    self.updatePlayPauseUI(isPlaying: false)

                case .waitingToPlayAtSpecifiedRate:
                    // Buffering - show loading indicator only if user didn't pause
                    if !self.isPausedByUser {
                        self.loadingIndicator.startAnimating()
                        // During user scrub/seek buffering we keep status as "playing" (no waiting event),
                        // but still show the native loading indicator.
                        if !self.isScrubbing && CACurrentMediaTime() >= self.suppressWaitingUntil {
                            self.send(.waiting)
                        }
                    }

                @unknown default:
                    break
                }
            }
        }

        // Listen for video end notification
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(videoDidEnd),
            name: .AVPlayerItemDidPlayToEndTime,
            object: item
        )
        NotificationCenter.default.addObserver(
            forName: .AVPlayerItemFailedToPlayToEndTime,
            object: item,
            queue: .main
        ) { [weak self] notification in
            guard let self = self else { return }
            guard currentSequence == self.loadingSequence else { return }
            let error = (notification.userInfo?[AVPlayerItemFailedToPlayToEndTimeErrorKey] as? NSError)?.localizedDescription ?? "unknown error"
            os_log("MediaPlayer item failed to play to end seq=%llu error=%{public}@", log: self.log, type: .error, currentSequence, error)
            self.send(.error(code: "play_failed", message: error))
            self.stopDisplayLink()
            self.revealVideoWorkItem?.cancel()
            self.waitingForFirstFrame = false
            self.firstFrameDisplayed = false
            self.loadingIndicator.stopAnimating()
            if let image = self.posterImageView.image, !image.size.equalTo(.zero) {
                self.posterImageView.isHidden = false
            }
        }

        // timeupdate event triggers every 250ms
        let interval = CMTime(seconds: 0.25, preferredTimescale: 600)
        timeObserver = player?.addPeriodicTimeObserver(forInterval: interval, queue: .main) { [weak self] time in
            guard let self = self else { return }
            Task { @MainActor [weak self] in
                guard let self = self else { return }
                guard currentSequence == self.loadingSequence else { return }

                self.sendProgress(time: time)
                self.updateProgressUI(time: time)
            }
        }

    }

    @objc private func videoDidEnd() {
        Task { @MainActor in
            os_log("MediaPlayer video ended (loop=%{public}@)", log: OSLog(subsystem: "LingXia", category: "Media"), type: .info, String(loopEnabled))

            // Cancel pending play event on end
            pendingPlayEvent = false

            if loopEnabled {
                // Loop: restart from beginning and continue playing without spinner
                waitingForFirstFrame = false
                firstFrameDisplayed = true
                pendingPosterHide = false
                playerLayer.opacity = 1
                playerLayer.isHidden = false
                posterImageView.isHidden = true
                posterImageView.alpha = 1

                let rate = Float(currentPlaybackRate)
                player?.seek(to: .zero, toleranceBefore: .zero, toleranceAfter: .zero) { [weak self] finished in
                    guard let self = self, finished, self.loopEnabled else { return }
                    DispatchQueue.main.async {
                        self.player?.playImmediately(atRate: rate)
                    }
                }
                send(.ended) // still emit ended per semantics
                return
            }

            // Show play button (pause state) and restore poster
            updatePlayPauseUI(isPlaying: false)

            // Show poster when video ends - reset all poster/layer states
            firstFrameDisplayed = false
            pendingPosterHide = false
            posterImageView.alpha = 1
            posterImageView.isHidden = false
            playerLayer.opacity = 0  // Hide video layer to show poster

            send(.ended)
        }
    }

    private func play() {
        isPausedByUser = false  // User wants to play
        if waitingForFirstFrame {
            desiredPlayWhenReady = true
            return
        }
        startPlaybackNow()
    }

    private func startPlaybackNow() {
        guard let player = player else { return }
        desiredPlayWhenReady = false
        if #available(iOS 15.0, *) {
            player.currentItem?.audioTimePitchAlgorithm = .timeDomain
        } else {
            player.currentItem?.audioTimePitchAlgorithm = .lowQualityZeroLatency
        }
        player.currentItem?.preferredForwardBufferDuration = 2
        player.automaticallyWaitsToMinimizeStalling = true

        // Ensure playerLayer is visible (may be hidden from screen lock)
        playerLayer.isHidden = false
        // Keep opacity at 0 - poster will be hidden when time actually progresses (like HarmonyOS)
        if !firstFrameDisplayed {
            playerLayer.opacity = 0
            pendingPosterHide = true
            lastTimeForPosterHide = -1
        }
        updatePlayPauseUI(isPlaying: true)

        player.playImmediately(atRate: Float(currentPlaybackRate))
        // Delay play event until video actually starts playing (time progresses in timeObserver)
        pendingPlayEvent = true
        lastTimeForPlayEvent = -1
        // Emit waiting event if we're waiting for first frame (buffering)
        if !firstFrameDisplayed {
            send(.waiting)
        }
        // Show controls on first play if configured (e.g., preview mode with autoplay)
        if shouldShowControlsOnFirstPlay {
            shouldShowControlsOnFirstPlay = false  // Only show once
            showControlsTemporarily()
        }
    }

    private func pause() {
        isPausedByUser = true  // User explicitly paused
        player?.pause()
        // Cancel pending play event if pause is called before video started
        pendingPlayEvent = false
        desiredPlayWhenReady = false
        // Always stop loading indicator when user explicitly pauses
        loadingIndicator.stopAnimating()
        send(.pause)
        updatePlayPauseUI(isPlaying: false)
        showControlsTemporarily()
    }

    private func stop() {
        player?.pause()
        player?.seek(to: .zero)
        // Cancel pending play event on stop
        pendingPlayEvent = false
        desiredPlayWhenReady = false
        send(.stop)
        updatePlayPauseUI(isPlaying: false)
        showControlsTemporarily()
        
        // Show poster image on stop and reset progress
        // Hide playerLayer to reveal poster underneath
        playerLayer.opacity = 0
        posterImageView.isHidden = false
        posterImageView.alpha = 1
        progressSlider.value = 0
        firstFrameDisplayed = false
    }

    private func seek(to seconds: Double) {
        guard let player = player else { return }
        guard let duration = player.currentItem?.duration.seconds,
              duration.isFinite, duration > 0 else { return }
        
        let clampedSeconds = max(0, min(seconds, duration))
        let time = CMTime(seconds: clampedSeconds, preferredTimescale: 600)
        
        isSeeking = true
        suppressWaitingUntil = CACurrentMediaTime() + 1.5
        
        // Update slider immediately to target position (prevents visual bounce)
        progressSlider.value = Float(clampedSeconds / duration)
        
        // Use precise seeking with zero tolerance for accurate positioning
        player.seek(to: time, toleranceBefore: .zero, toleranceAfter: .zero) { [weak self] finished in
            guard let self = self else { return }
            DispatchQueue.main.async {
                // Keep isSeeking true briefly to let time observer settle
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                    self.isSeeking = false
                }
                
                guard finished else { return }
                self.send(.seeked(time: clampedSeconds))
            }
        }
    }

    private func setPlaybackRate(_ rate: Double) {
        guard let player = player else { return }

        currentPlaybackRate = rate

        // Improve audio quality when speeding up
        if #available(iOS 15.0, *) {
            player.currentItem?.audioTimePitchAlgorithm = .timeDomain
        } else {
            player.currentItem?.audioTimePitchAlgorithm = .lowQualityZeroLatency
        }
        player.currentItem?.preferredForwardBufferDuration = 2
        player.automaticallyWaitsToMinimizeStalling = true

        let floatRate = Float(rate)
        if waitingForFirstFrame {
            desiredPlayWhenReady = true
        } else {
            player.playImmediately(atRate: floatRate)
        }

        // Verify if it actually worked (some sources may not support it)
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) { [weak self] in
            guard let self = self, let player = self.player else { return }
            let actualRate = Double(player.rate)
            if abs(actualRate - rate) > 0.01 {
                self.currentPlaybackRate = actualRate
                self.send(.error(code: "RATE_NOT_SUPPORTED", message: "Current source does not support this playback rate"))
            } else {
                self.currentPlaybackRate = actualRate
                self.send(.rateChange(rate: actualRate))
                self.updateSettingsMenu()
            }
        }
    }

    private func sendProgress(time: CMTime) {
        guard let duration = player?.currentItem?.duration.seconds,
              duration.isFinite, duration > 0 else { return }
        let currentTime = CMTimeGetSeconds(time)

        // Hide poster when video is actually rendering frames
        // This prevents black screen flash by waiting for time to progress
        if pendingPosterHide {
            let timeAdvanced = lastTimeForPosterHide >= 0 && currentTime > lastTimeForPosterHide
            if currentTime > 0.2 && timeAdvanced {
                pendingPosterHide = false
                firstFrameDisplayed = true

                // Always stop loading indicator when video is ready
                loadingIndicator.stopAnimating()

                // Show video layer
                playerLayer.opacity = 1

                // Crossfade poster to video (if poster was visible)
                if !posterImageView.isHidden {
                    UIView.animate(withDuration: 0.2, delay: 0, options: [.curveEaseOut]) {
                        self.posterImageView.alpha = 0
                    } completion: { _ in
                        self.posterImageView.isHidden = true
                        self.posterImageView.alpha = 1
                    }
                }
            }
            lastTimeForPosterHide = currentTime
        }

        // Emit play event when video actually starts playing (time progresses)
        if pendingPlayEvent {
            let timeAdvanced = lastTimeForPlayEvent >= 0 && currentTime > lastTimeForPlayEvent
            if currentTime > 0 && timeAdvanced {
                os_log("MediaPlayer emitting delayed play event, currentTime=%.2f, lastTime=%.2f", log: log, type: .info, currentTime, lastTimeForPlayEvent)
                pendingPlayEvent = false
                send(.play)
            }
            lastTimeForPlayEvent = currentTime
        }

        send(.timeUpdate(currentTime: currentTime, duration: duration))
    }

    private func send(_ event: LxMediaEvent) {
        rawEventSink(event.rawPayload)
        typedEventSink?(event)
    }

    private func replaceActivePipe(with pipe: LxMediaPipe?) {
        if let existing = activePipe, existing.shouldUnlinkOnClose, existing.url != pipe?.url {
            existing.close()
        }
        activePipe = pipe
    }

    // MARK: Overlay UI

    private func setupOverlayUI() {
        overlayView.frame = view.bounds
        overlayView.backgroundColor = UIColor.clear
        overlayView.isUserInteractionEnabled = true
        view.addSubview(overlayView)
        container.overlay = overlayView

        // Tap catcher for empty areas
        tapCatcher.backgroundColor = .clear
        tapCatcher.addTarget(self, action: #selector(handleTap), for: .touchUpInside)
        tapCatcher.isUserInteractionEnabled = true
        tapCatcher.showsTouchWhenHighlighted = false
        overlayView.addSubview(tapCatcher)
        overlayView.tapTarget = tapCatcher

        // Top gradient
        topGradient.colors = [
            UIColor.black.withAlphaComponent(0.6).cgColor,
            UIColor.clear.cgColor
        ]
        overlayView.layer.addSublayer(topGradient)

        // Bottom gradient
        bottomGradient.colors = [
            UIColor.clear.cgColor,
            UIColor.black.withAlphaComponent(0.7).cgColor
        ]
        overlayView.layer.addSublayer(bottomGradient)

        // Top bar
        topBar.backgroundColor = .clear
        overlayView.addSubview(topBar)

        // Close/Back button (controlled by showCloseButton config)
        backButton.setImage(LxIcon.image(named: "icon_close"), for: .normal)
        backButton.tintColor = .white
        backButton.addTarget(self, action: #selector(handleBackTap), for: .touchUpInside)
        backButton.contentEdgeInsets = UIEdgeInsets(top: 8, left: 8, bottom: 8, right: 8)
        backButton.isHidden = true // Will be shown based on config
        topBar.addSubview(backButton)

        // Title
        titleLabel.textColor = .white
        titleLabel.font = UIFont.systemFont(ofSize: 16, weight: .semibold)
        titleLabel.text = ""
        topBar.addSubview(titleLabel)

        // Bottom bar
        bottomBar.backgroundColor = .clear
        overlayView.addSubview(bottomBar)

        // Progress slider
        progressSlider.minimumTrackTintColor = UIColor(red: 0.0, green: 0.48, blue: 1.0, alpha: 1.0)
        progressSlider.maximumTrackTintColor = UIColor.white.withAlphaComponent(0.3)
        progressSlider.setThumbImage(createThumbImage(), for: .normal)
        progressSlider.isEnabled = true
        progressSlider.isUserInteractionEnabled = true
        progressSlider.addTarget(self, action: #selector(handleSliderTouchDown), for: .touchDown)
        progressSlider.addTarget(self, action: #selector(handleSliderChange), for: .valueChanged)
        progressSlider.addTarget(self, action: #selector(handleSliderTouchUp), for: [.touchUpInside, .touchUpOutside, .touchCancel])
        
        // Setup direct scrub callbacks for DebugSlider (bypasses target-action for faster response)
        progressSlider.onScrubStart = { [weak self] in
            self?.handleSliderScrubStart()
        }
        progressSlider.onScrub = { [weak self] value in
            self?.handleSliderScrub(value: value)
        }
        progressSlider.onSeekComplete = { [weak self] value in
            self?.handleSliderSeekComplete(value: value)
        }
        progressSlider.onScrubEnd = { [weak self] in
            self?.handleSliderScrubEnd()
        }
        
        bottomBar.addSubview(progressSlider)

        // Time label (shows remaining time)
        timeLabel.textColor = .white
        timeLabel.font = UIFont.monospacedDigitSystemFont(ofSize: 12, weight: .semibold)
        timeLabel.text = "0:00"
        timeLabel.textAlignment = .center
        timeLabel.layer.shadowColor = UIColor.black.cgColor
        timeLabel.layer.shadowOffset = CGSize(width: 0, height: 1)
        timeLabel.layer.shadowOpacity = 0.5
        timeLabel.layer.shadowRadius = 2
        bottomBar.addSubview(timeLabel)

        // Play/Pause button
        playPauseButton.setImage(LxIcon.image(named: "icon_play"), for: .normal)
        playPauseButton.tintColor = .white
        playPauseButton.addTarget(self, action: #selector(handlePlayPauseTap), for: .touchUpInside)
        playPauseButton.contentEdgeInsets = UIEdgeInsets(top: 8, left: 8, bottom: 8, right: 8)
        bottomBar.addSubview(playPauseButton)

        // Volume button
        volumeButton.setImage(LxIcon.image(named: "icon_volume_on"), for: .normal)
        volumeButton.tintColor = .white
        volumeButton.addTarget(self, action: #selector(handleVolumeTap), for: .touchUpInside)
        volumeButton.contentEdgeInsets = UIEdgeInsets(top: 8, left: 8, bottom: 8, right: 8)
        bottomBar.addSubview(volumeButton)

        // Volume slider
        volumeSlider.minimumValue = 0
        volumeSlider.maximumValue = 1
        volumeSlider.value = 1
        volumeSlider.minimumTrackTintColor = .white
        volumeSlider.maximumTrackTintColor = UIColor.white.withAlphaComponent(0.3)
        volumeSlider.setThumbImage(createThumbImage(), for: .normal)
        volumeSlider.addTarget(self, action: #selector(handleVolumeChange), for: .valueChanged)
        bottomBar.addSubview(volumeSlider)

        // Settings button (collapsed menu for quality/speed)
        settingsButton.setImage(LxIcon.image(named: "icon_settings"), for: .normal)
        settingsButton.tintColor = .white
        settingsButton.backgroundColor = .clear
        settingsButton.isHidden = true
        settingsButton.isUserInteractionEnabled = true
        settingsButton.contentEdgeInsets = UIEdgeInsets(top: 8, left: 8, bottom: 8, right: 8)
        settingsButton.addTarget(self, action: #selector(handleSettingsTap), for: .touchUpInside)
        bottomBar.addSubview(settingsButton)

        // Fullscreen button
        fullscreenButton.setImage(LxIcon.image(named: "icon_fullscreen_enter"), for: .normal)
        fullscreenButton.tintColor = .white
        fullscreenButton.addTarget(self, action: #selector(handleFullscreenTap), for: .touchUpInside)
        fullscreenButton.contentEdgeInsets = UIEdgeInsets(top: 8, left: 8, bottom: 8, right: 8)
        bottomBar.addSubview(fullscreenButton)

        // Loading indicator
        loadingIndicator.color = .white
        loadingIndicator.hidesWhenStopped = true
        overlayView.addSubview(loadingIndicator)

        // Note: Using tapCatcher button instead of gesture recognizer
        // This prevents interference with UISlider drag operations

        // Initial state - hide controls
        topBar.alpha = 0
        bottomBar.alpha = 0
        topBar.isUserInteractionEnabled = false
        bottomBar.isUserInteractionEnabled = false
        topGradient.opacity = 0
        bottomGradient.opacity = 0
        controlsVisible = false

        // Set initial button visibility based on config
        updateFullscreenButtonVisibility()

        // App lifecycle observers (pause/resume UI sync)
        NotificationCenter.default.addObserver(self, selector: #selector(handleAppWillResignActive), name: UIApplication.willResignActiveNotification, object: nil)
        NotificationCenter.default.addObserver(self, selector: #selector(handleAppDidBecomeActive), name: UIApplication.didBecomeActiveNotification, object: nil)
    }

    private func createThumbImage() -> UIImage {
        let size = CGSize(width: 16, height: 16)
        let renderer = UIGraphicsImageRenderer(size: size)
        return renderer.image { ctx in
            ctx.cgContext.setFillColor(UIColor.white.cgColor)
            ctx.cgContext.addEllipse(in: CGRect(origin: .zero, size: size))
            ctx.cgContext.fillPath()

            ctx.cgContext.setShadow(offset: CGSize(width: 0, height: 1), blur: 2, color: UIColor.black.withAlphaComponent(0.3).cgColor)
        }
    }

    private func layoutOverlay() {
        let bounds = view.bounds
        overlayView.frame = bounds
        tapCatcher.frame = bounds
        posterImageView.frame = bounds

        // In fullscreen with transform rotation, manually calculate safe areas
        let safeTop: CGFloat = 0
        let safeBottom: CGFloat = 0
        let safeLeading: CGFloat = isFullscreen ? 44 : 0  // Notch area when rotated
        let safeTrailing: CGFloat = 0

        // Top gradient
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        topGradient.frame = CGRect(x: 0, y: 0, width: bounds.width, height: 100)
        bottomGradient.frame = CGRect(x: 0, y: bounds.height - 120, width: bounds.width, height: 120)
        CATransaction.commit()

        // Top bar layout
        let topBarHeight: CGFloat = 50
        topBar.frame = CGRect(x: 0, y: safeTop, width: bounds.width, height: topBarHeight)

        backButton.frame = CGRect(x: 8 + safeLeading, y: 5, width: 44, height: 40)
        titleLabel.frame = CGRect(x: 60 + safeLeading, y: 5, width: bounds.width - 120 - safeLeading - safeTrailing, height: 40)

        // Bottom bar layout - compact 2 rows like HarmonyOS
        let bottomBarHeight: CGFloat = 80  // Reduced from 100
        bottomBar.frame = CGRect(x: 0, y: bounds.height - bottomBarHeight - safeBottom, width: bounds.width, height: bottomBarHeight)

        let padding: CGFloat = 12 + safeLeading  // Reduced from 16
        let buttonWidth: CGFloat = 40  // Reduced from 44

        // Row 1: Progress slider + Time label
        let progressY: CGFloat = 10
        timeLabel.sizeToFit()
        let timeLabelWidth: CGFloat = 50
        let timeLabelPadding: CGFloat = 8  // Small padding from edge

        // Time label on the right side - closer to edge
        timeLabel.frame = CGRect(
            x: bounds.width - timeLabelPadding - safeTrailing - timeLabelWidth,
            y: progressY + 1,
            width: timeLabelWidth,
            height: 20
        )

        // Progress slider - smaller gap to time label (4px instead of 8)
        progressSlider.frame = CGRect(
            x: padding,
            y: progressY,
            width: bounds.width - padding - safeTrailing - timeLabelWidth - timeLabelPadding - 4,
            height: 20
        )

        // Row 2: Controls (Play + Volume + Volume Slider ... Settings + Fullscreen)
        let controlY: CGFloat = 36  // Reduced from 45
        let spacing: CGFloat = 8  // Reduced from 12
        let volumeSliderWidth: CGFloat = 60  // Reduced from 80
        let settingsSize: CGFloat = 32  // Reduced from 36

        // Left side: Play + Volume + Volume Slider
        var leftX = padding

        // Play/Pause button
        playPauseButton.frame = CGRect(x: leftX, y: controlY, width: buttonWidth, height: buttonWidth)
        leftX += buttonWidth + spacing

        // Volume button
        if !volumeButton.isHidden {
            volumeButton.frame = CGRect(x: leftX, y: controlY, width: buttonWidth, height: buttonWidth)
            leftX += buttonWidth + 4
        } else {
            volumeButton.frame = .zero
        }

        // Volume slider (horizontal, next to volume button)
        if !volumeSlider.isHidden {
            volumeSlider.frame = CGRect(
                x: leftX,
                y: controlY + (buttonWidth - 20) / 2,
                width: volumeSliderWidth,
                height: 20
            )
        } else {
            volumeSlider.frame = .zero
        }

        // Right side: Fullscreen + Settings (from right to left)
        var trailingX = bounds.width - padding - safeTrailing

        // Fullscreen button (rightmost if visible)
        if !fullscreenButton.isHidden {
            trailingX -= buttonWidth
            fullscreenButton.frame = CGRect(
                x: trailingX,
                y: controlY,
                width: buttonWidth,
                height: buttonWidth
            )
            trailingX -= spacing
        } else {
            fullscreenButton.frame = .zero
        }

        // Settings button (left of fullscreen)
        if !settingsButton.isHidden {
            trailingX -= settingsSize
            settingsButton.frame = CGRect(
                x: trailingX,
                y: controlY + (buttonWidth - settingsSize) / 2,
                width: settingsSize,
                height: settingsSize
            )
            trailingX -= spacing
        } else {
            settingsButton.frame = .zero
        }

        // Loading indicator and center play button centered
        let centerSize: CGFloat = 80
        let centerFrame = CGRect(
            x: (bounds.width - centerSize) / 2,
            y: (bounds.height - centerSize) / 2,
            width: centerSize,
            height: centerSize
        )
        loadingIndicator.frame = centerFrame
    }

    private func updatePlayPauseUI(isPlaying: Bool) {
        let iconName = isPlaying ? "icon_pause" : "icon_play"
        playPauseButton.setImage(LxIcon.image(named: iconName), for: .normal)
    }

    private func updateProgressUI(time: CMTime) {
        // If controls are disabled, no need to update UI
        guard controlsEnabled else { return }
        // Skip slider update during scrubbing or seeking to avoid overwriting position
        guard !isScrubbing, !isSeeking else { return }
        guard let durationSeconds = player?.currentItem?.duration.seconds,
              durationSeconds.isFinite,
              durationSeconds > 0 else { return }
        let current = CMTimeGetSeconds(time)
        let remaining = durationSeconds - current
        // Show remaining time with minus sign
        timeLabel.text = "-" + formatTime(remaining)
        progressSlider.value = Float(current / durationSeconds)
    }

    private func formatTime(_ seconds: Double) -> String {
        let intVal = Int(seconds)
        let m = intVal / 60
        let s = intVal % 60
        if m >= 60 {
            let h = m / 60
            let m = m % 60
            return String(format: "%d:%02d:%02d", h, m, s)
        }
        return String(format: "%d:%02d", m, s)
    }

    private func updateControlsVisibility() {
        // HTML5 controls: when disabled, hide ALL control UI elements
        let hideControls = !controlsEnabled
        topBar.isHidden = hideControls
        bottomBar.isHidden = hideControls
    }

    private func updateCloseButtonVisibility() {
        // Show close button only if configured (for preview mode)
        // In fullscreen, never show it (use system back gesture instead)
        backButton.isHidden = !showCloseButton || isFullscreen
    }

    private func updateFullscreenButtonVisibility() {
        // Fullscreen button hidden if disabled or already in fullscreen
        fullscreenButton.isHidden = !showFullscreenButton || isFullscreen
    }

    private func showControlsTemporarily() {
        controlsHideWorkItem?.cancel()

        UIView.animate(withDuration: 0.3, delay: 0, options: [.curveEaseOut], animations: {
            self.topBar.alpha = 1
            self.bottomBar.alpha = 1
            self.topGradient.opacity = 1
            self.bottomGradient.opacity = 1
        })

        topBar.isUserInteractionEnabled = true
        bottomBar.isUserInteractionEnabled = true
        controlsVisible = true

        let work = DispatchWorkItem { [weak self] in
            guard let self = self else { return }
            self.controlsVisible = false
            UIView.animate(withDuration: 0.3, delay: 0, options: [.curveEaseIn], animations: {
                self.topBar.alpha = 0
                self.bottomBar.alpha = 0
                self.topGradient.opacity = 0
                self.bottomGradient.opacity = 0
            }, completion: { _ in
                // Only disable interaction after animation completes
                self.topBar.isUserInteractionEnabled = false
                self.bottomBar.isUserInteractionEnabled = false
            })
        }
        controlsHideWorkItem = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 3, execute: work)
    }

    // MARK: Settings (Quality & Speed)

    private func updateSettingsMenu() {

        // Initialize defaults if needed
        if let firstLabel = availableQualities.first?.label, currentQuality == nil {
            currentQuality = firstLabel
        }
        if !availablePlaybackRates.isEmpty,
           !availablePlaybackRates.contains(currentPlaybackRate) {
            currentPlaybackRate = availablePlaybackRates.first ?? 1.0
        }

        var menuSections: [UIMenu] = []

        // Quality section
        if !availableQualities.isEmpty {
            let qualityActions = availableQualities.enumerated().map { index, quality -> UIAction in
                let label = quality.label
                let state: UIMenuElement.State = (label == currentQuality) ? .on : .off
                let icon = state == .on ? UIImage(systemName: "checkmark") : nil
                return UIAction(title: label, image: icon, state: state) { [weak self] _ in
                    self?.handleQualitySelection(label: label)
                }
            }
            let qualitySection = UIMenu(
                title: L10n.string("lx_video_quality"),
                image: UIImage(systemName: "video.fill"),
                options: [.singleSelection],
                children: qualityActions
            )
            menuSections.append(qualitySection)
        }

        // Speed section
        if !availablePlaybackRates.isEmpty {
            let speedActions = availablePlaybackRates.map { rate -> UIAction in
                let state: UIMenuElement.State = rate == currentPlaybackRate ? .on : .off
                let icon = state == .on ? UIImage(systemName: "checkmark") : nil
                return UIAction(title: formattedRate(rate), image: icon, state: state) { [weak self] _ in
                    self?.handleSpeedSelection(rate: rate)
                }
            }
            let speedSection = UIMenu(
                title: L10n.string("lx_video_speed"),
                image: UIImage(systemName: "gauge.with.dots.needle.50percent"),
                options: [.singleSelection],
                children: speedActions
            )
            menuSections.append(speedSection)
        }

        // Update settings button - always use custom popup for consistency
        if menuSections.isEmpty {
            settingsButton.isHidden = true
            settingsButton.isEnabled = false
        } else {
            settingsButton.isHidden = false
            settingsButton.isEnabled = true
            // Ensure it's above other bottom controls
            bottomBar.bringSubviewToFront(settingsButton)
        }

        // No UIMenu - always use custom popup via handleSettingsTap
        settingsButton.menu = nil
        settingsButton.showsMenuAsPrimaryAction = false

        // Relayout to ensure the settings button is positioned correctly
        DispatchQueue.main.async { [weak self] in
            self?.layoutOverlay()
        }
    }

    private func handleQualitySelection(label: String) {
        currentQuality = label
        os_log("MediaPlayer quality selected: %{public}@", log: log, type: .info, label)
        
        let selectedQuality = availableQualities.first(where: { $0.label == label })
        let switchedUrl = selectedQuality?.url?.absoluteString

        if let url = selectedQuality?.url, let player {
            os_log("MediaPlayer switching to internal URL: %{public}@", log: log, type: .info, url.absoluteString)
            pendingRestoreAfterLoad = (player.currentTime(), player.rate > 0)
            loadVideo(url: url)
        }
	        
        send(.qualityChange(quality: label, url: switchedUrl))
        updateSettingsMenu()
    }

    private func handleSpeedSelection(rate: Double) {
        // Just set the rate - setPlaybackRate handles the logic and event emission
        setPlaybackRate(rate)
        // Note: setPlaybackRate emits .rateChange
    }

    private func formattedRate(_ rate: Double) -> String {
        if rate.truncatingRemainder(dividingBy: 1) == 0 {
            return String(format: "%.0fx", rate)
        }
        // Use %g to remove trailing zeros (1.5x instead of 1.50x)
        return String(format: "%gx", rate)
    }

    private func revealVideoIfReady(progressTime: Double? = nil, reason: String, sequence: UInt64) {
        guard waitingForFirstFrame else { return }
        guard sequence == loadingSequence else {
            os_log("MediaPlayer IGNORE stale revealVideoIfReady seq=%llu (current=%llu)", log: log, type: .debug, sequence, loadingSequence)
            return
        }

        // Only reveal when we have pixel buffer - THE RELIABLE SIGNAL
        guard reason == "pixelBuffer" else {
            return
        }

        performRevealVideo(reason: reason, sequence: sequence)
    }

    private func forceRevealVideo(sequence: UInt64) {
        guard waitingForFirstFrame else { return }
        guard sequence == loadingSequence else {
            os_log("MediaPlayer IGNORE stale forceRevealVideo seq=%llu (current=%llu)", log: log, type: .debug, sequence, loadingSequence)
            return
        }

        // Safety check: verify layer readiness; retry a few times before forcing reveal
        if !playerLayer.isReadyForDisplay {
            if forceRevealAttempts < 3 {
                forceRevealAttempts += 1
                let delay = 0.2 * Double(forceRevealAttempts)
                os_log("MediaPlayer timeout but layer not ready, retry %d after %.1fs", log: log, type: .info, forceRevealAttempts, delay)
                DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
                    self?.forceRevealVideo(sequence: sequence)
                }
                return
            } else {
                os_log("MediaPlayer still not ready after %d retries; keep waiting", log: log, type: .error, forceRevealAttempts)
                if let image = posterImageView.image, !image.size.equalTo(.zero) {
                    posterImageView.isHidden = false
                    posterImageView.alpha = 1
                }
                scheduleRevealTimeout(sequence: sequence, delay: 1.0)
                return
            }
        }
        forceRevealAttempts = 0

        performRevealVideo(reason: "timeout", sequence: sequence)
    }

    private func performRevealVideo(reason: String, sequence: UInt64) {
        guard sequence == loadingSequence else {
            os_log("MediaPlayer IGNORE stale performRevealVideo seq=%llu (current=%llu)", log: log, type: .debug, sequence, loadingSequence)
            return
        }
        // Stop all frame detection mechanisms
        revealVideoWorkItem?.cancel()
        revealVideoWorkItem = nil
        stopDisplayLink()

        os_log("MediaPlayer reveal video seq=%llu (reason=%{public}@)", log: log, type: .info, sequence, reason)

        CATransaction.begin()
        CATransaction.setDisableActions(true)
        playerLayer.isHidden = false
        CATransaction.commit()

        waitingForFirstFrame = false

        // Don't set firstFrameDisplayed here - let sendProgress handle it
        // This ensures poster is only hidden when video time actually progresses
        // (prevents black screen flash)

        // Start playback - poster hiding will happen in sendProgress when time progresses
        let shouldPlay = desiredPlayWhenReady
        if shouldPlay {
            startPlaybackNow()
        } else {
            // If not playing, just enable pending poster hide for when playback starts
            pendingPosterHide = true
            lastTimeForPosterHide = -1
        }
    }

    @objc private func handleAppWillResignActive() {
        wasPlayingBeforeBackground = (player?.timeControlStatus == .playing) || ((player?.rate ?? 0) > 0.01)
        if wasPlayingBeforeBackground {
            pause()
        } else {
            updatePlayPauseUI(isPlaying: false)
        }
        firstFrameDisplayed = false
        waitingForFirstFrame = true
        desiredPlayWhenReady = wasPlayingBeforeBackground
        playerLayer.isHidden = false
        playerLayer.opacity = 0
        if posterImageView.image != nil {
            posterImageView.isHidden = false
            posterImageView.alpha = 1
        }
    }

    @objc private func handleAppDidBecomeActive() {
        let isPlaying = (player?.timeControlStatus == .playing) || ((player?.rate ?? 0) > 0.01)
        updatePlayPauseUI(isPlaying: isPlaying)
        wasPlayingBeforeBackground = false

        // If we were waiting for a frame (e.g. after returning from background), re-arm detection
        // so a programmatic play resumes without requiring a tap.
        if waitingForFirstFrame {
            armFrameDetection(sequence: loadingSequence)
            checkPixelBufferAvailability(sequence: loadingSequence)
            if playerLayer.isReadyForDisplay {
                forceRevealVideo(sequence: loadingSequence)
            }
        }
    }

    // MARK: - Frame Detection (Reliable Signal)

    @objc private func displayLinkDidRefresh() {
        guard waitingForFirstFrame else {
            stopDisplayLink()
            return
        }

        checkPixelBufferAvailability(sequence: loadingSequence)
    }

    private func checkPixelBufferAvailability(sequence: UInt64) {
        guard waitingForFirstFrame else { return }
        guard let output = videoOutput else { return }

        let itemTime = output.itemTime(forHostTime: CACurrentMediaTime())

        // This is THE reliable signal: actual pixel buffer is available
        if output.hasNewPixelBuffer(forItemTime: itemTime) {
            os_log("MediaPlayer FIRST FRAME DETECTED via videoOutput seq=%llu", log: log, type: .info, sequence)
            stopDisplayLink()
            revealVideoIfReady(progressTime: itemTime.seconds, reason: "pixelBuffer", sequence: sequence)
        }
    }

    private func stopDisplayLink() {
        displayLink?.invalidate()
        displayLink = nil
    }

    private func scheduleRevealTimeout(sequence: UInt64, delay: TimeInterval = 1.5) {
        revealVideoWorkItem?.cancel()
        let work = DispatchWorkItem { [weak self] in
            guard let self = self else { return }
            guard sequence == self.loadingSequence else {
                os_log("MediaPlayer IGNORE stale timeout seq=%llu (current=%llu)", log: self.log, type: .debug, sequence, self.loadingSequence)
                return
            }
            if self.waitingForFirstFrame {
                os_log("MediaPlayer force reveal after timeout seq=%llu", log: self.log, type: .info, sequence)
                self.forceRevealVideo(sequence: sequence)
            }
        }
        revealVideoWorkItem = work
        DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: work)
    }

    private func armFrameDetection(sequence: UInt64) {
        if waitingForFirstFrame {
            // Already armed for this sequence
            return
        }
        waitingForFirstFrame = true
        startDisplayLink(sequence: sequence)
        scheduleRevealTimeout(sequence: sequence)
        loadingIndicator.startAnimating()
        overlayView.bringSubviewToFront(loadingIndicator)
    }

    private func startDisplayLink(sequence: UInt64) {
        stopDisplayLink()
        let link = CADisplayLink(target: self, selector: #selector(displayLinkDidRefresh))
        link.add(to: .main, forMode: .common)
        displayLink = link
        os_log("MediaPlayer started displayLink for frame detection seq=%llu", log: log, type: .debug, sequence)
    }

    // MARK: - UI actions
    @objc private func handleTap() {
        if controlsVisible {
            controlsHideWorkItem?.cancel()
            controlsVisible = false
            UIView.animate(withDuration: 0.3, animations: {
                self.topBar.alpha = 0
                self.bottomBar.alpha = 0
                self.topGradient.opacity = 0
                self.bottomGradient.opacity = 0
            }, completion: { _ in
                // Only disable interaction after animation completes
                self.topBar.isUserInteractionEnabled = false
                self.bottomBar.isUserInteractionEnabled = false
            })
        } else {
            showControlsTemporarily()
        }
    }

    @objc private func handlePlayPauseTap() {
        if player?.timeControlStatus == .playing {
            pause()
        } else {
            // User explicitly tapped play - always allow playback
            // Reset waiting state if player has valid item loaded
            if player?.currentItem != nil {
                waitingForFirstFrame = false
            }

            // Check if video ended (at the end position)
            if let duration = player?.currentItem?.duration,
               let currentTime = player?.currentTime(),
               duration.isValid && !duration.isIndefinite,
               abs(CMTimeGetSeconds(currentTime) - CMTimeGetSeconds(duration)) < 0.5 {
                // Video ended, restart from beginning
                player?.seek(to: .zero)
            }
            play()
        }
    }

    @objc private func handleFullscreenTap() {
        // Fullscreen is always allowed unless controls are disabled
        if isFullscreen {
            exitFullscreen()
        } else {
            enterFullscreen()
        }
        showControlsTemporarily()
    }

    @objc private func handleBackTap() {
        if isFullscreen {
            exitFullscreen()
        }
    }

    @objc private func handleSettingsTap() {
        guard !availableQualities.isEmpty || !availablePlaybackRates.isEmpty else { return }

        // Toggle: if already showing, dismiss it
        if view.subviews.contains(where: { $0.tag == 9998 }) {
            dismissSettingsPopup()
            return
        }

        // Show custom beautiful popup (consistent experience)
        showSettingsPopup()
        showControlsTemporarily()
    }

    private func showSettingsPopup() {
        // Prevent duplicate popups
        if view.subviews.contains(where: { $0.tag == 9998 }) {
            return
        }

        var yOffset: CGFloat = 8
        let menuWidth: CGFloat = 180

        let hasQuality = !availableQualities.isEmpty
        let hasSpeed = !availablePlaybackRates.isEmpty

        // Create popup with solid background (simpler and more reliable for touch)
        let popup = UIView()
        popup.backgroundColor = UIColor(white: 0.18, alpha: 0.97)  // Dark semi-transparent
        popup.layer.cornerRadius = 13
        popup.clipsToBounds = true

        // Add subtle border and shadow for depth
        popup.layer.borderColor = UIColor.white.withAlphaComponent(0.15).cgColor
        popup.layer.borderWidth = 0.5
        popup.layer.shadowColor = UIColor.black.cgColor
        popup.layer.shadowOffset = CGSize(width: 0, height: 4)
        popup.layer.shadowRadius = 12
        popup.layer.shadowOpacity = 0.4

        // Quality option
        if hasQuality {
            let currentQualityText = currentQuality ?? "Auto"
            let button = createMainMenuButton(
                title: L10n.string("lx_video_quality"),
                subtitle: currentQualityText,
                action: #selector(handleShowQualitySubmenu)
            )
            button.frame = CGRect(x: 8, y: yOffset, width: menuWidth - 16, height: 44)
            popup.addSubview(button)
            yOffset += 44
        }

        // Speed option
        if hasSpeed {
            let button = createMainMenuButton(
                title: L10n.string("lx_video_speed"),
                subtitle: formattedRate(currentPlaybackRate),
                action: #selector(handleShowSpeedSubmenu)
            )
            button.frame = CGRect(x: 8, y: yOffset, width: menuWidth - 16, height: 44)
            popup.addSubview(button)
            yOffset += 44
        }

        yOffset += 8

        // Size popup
        popup.frame = CGRect(x: 0, y: 0, width: menuWidth, height: yOffset)

        // Position near settings button
        let settingsFrame = settingsButton.convert(settingsButton.bounds, to: view)
        var popupX: CGFloat
        var popupY: CGFloat

        if isFullscreen {
            // In fullscreen (landscape), show to the left of settings button
            popupX = settingsFrame.minX - menuWidth - 16
            popupY = max(20, min(settingsFrame.midY - popup.frame.height / 2, view.bounds.height - popup.frame.height - 20))
        } else {
            // In normal mode (portrait), show above settings button
            popupX = max(16, min(settingsFrame.midX - menuWidth / 2, view.bounds.width - menuWidth - 16))
            popupY = settingsFrame.minY - popup.frame.height - 12
        }
        popupX = max(12, min(popupX, view.bounds.width - menuWidth - 12))
        popupY = max(12, min(popupY, view.bounds.height - popup.frame.height - 12))
        popup.frame.origin = CGPoint(x: popupX, y: popupY)

        // Add to view with tap-to-dismiss overlay
        let overlay = UIButton(type: .custom)
        overlay.frame = view.bounds
        overlay.backgroundColor = UIColor.black.withAlphaComponent(0.3)
        overlay.tag = 9999
        overlay.addTarget(self, action: #selector(dismissSettingsPopup), for: .touchUpInside)

        // Add overlay and popup
        view.addSubview(overlay)
        view.addSubview(popup)
        popup.tag = 9998

        // Beautiful spring animation
        let initialTransform: CGAffineTransform
        if isFullscreen {
            initialTransform = CGAffineTransform(scaleX: 0.8, y: 0.8).concatenating(CGAffineTransform(translationX: 40, y: 0))
        } else {
            initialTransform = CGAffineTransform(scaleX: 0.8, y: 0.8).concatenating(CGAffineTransform(translationX: 0, y: 20))
        }
        popup.transform = initialTransform
        popup.alpha = 0
        overlay.alpha = 0

        UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0.5, options: [.curveEaseOut]) {
            popup.transform = .identity
            popup.alpha = 1
            overlay.alpha = 1
        }
    }

    private func createMainMenuButton(title: String, subtitle: String, action: Selector) -> UIButton {
        let button = UIButton(type: .custom)
        button.addTarget(self, action: action, for: .touchUpInside)
        button.backgroundColor = UIColor.white.withAlphaComponent(0.08)
        button.layer.cornerRadius = 10

        // Title
        let titleLabel = UILabel()
        titleLabel.text = title
        titleLabel.font = UIFont.systemFont(ofSize: 15, weight: .medium)
        titleLabel.textColor = .white
        titleLabel.frame = CGRect(x: 12, y: 8, width: 100, height: 18)
        titleLabel.isUserInteractionEnabled = false
        button.addSubview(titleLabel)

        // Subtitle (current value)
        let subtitleLabel = UILabel()
        subtitleLabel.text = subtitle
        subtitleLabel.font = UIFont.systemFont(ofSize: 12, weight: .regular)
        subtitleLabel.textColor = UIColor.white.withAlphaComponent(0.6)
        subtitleLabel.frame = CGRect(x: 12, y: 24, width: 100, height: 14)
        subtitleLabel.isUserInteractionEnabled = false
        button.addSubview(subtitleLabel)

        // Chevron
        let chevron = UIImageView(image: UIImage(systemName: "chevron.right", withConfiguration: UIImage.SymbolConfiguration(pointSize: 12, weight: .semibold)))
        chevron.tintColor = UIColor.white.withAlphaComponent(0.4)
        chevron.frame = CGRect(x: 144, y: 16, width: 12, height: 12)
        chevron.isUserInteractionEnabled = false
        button.addSubview(chevron)

        return button
    }

    private func createCompactMenuButton(title: String, isSelected: Bool, tag: Int, action: Selector) -> UIButton {
        let button = UIButton(type: .system)
        button.tag = tag
        button.addTarget(self, action: action, for: .touchUpInside)

        // Background for selected state
        if isSelected {
            button.backgroundColor = UIColor.systemBlue.withAlphaComponent(0.2)
            button.layer.cornerRadius = 8
        }

        // Title
        button.setTitle(title, for: .normal)
        button.setTitleColor(.white, for: .normal)
        button.titleLabel?.font = UIFont.systemFont(ofSize: 15, weight: isSelected ? .medium : .regular)
        button.contentHorizontalAlignment = .left
        button.titleEdgeInsets = UIEdgeInsets(top: 0, left: 12, bottom: 0, right: 32)

        // Checkmark for selected
        if isSelected {
            let checkmark = UIImageView(image: UIImage(systemName: "checkmark", withConfiguration: UIImage.SymbolConfiguration(pointSize: 13, weight: .semibold)))
            checkmark.translatesAutoresizingMaskIntoConstraints = false
            checkmark.tintColor = .systemBlue
            button.addSubview(checkmark)
            NSLayoutConstraint.activate([
                checkmark.centerYAnchor.constraint(equalTo: button.centerYAnchor),
                checkmark.trailingAnchor.constraint(equalTo: button.trailingAnchor, constant: -12),
                checkmark.widthAnchor.constraint(equalToConstant: 16),
                checkmark.heightAnchor.constraint(equalToConstant: 16)
            ])
        }

        return button
    }

    @objc private func dismissSettingsPopup() {
        let overlay = view.subviews.first(where: { $0.tag == 9999 })
        let popup = view.subviews.first(where: { $0.tag == 9998 })

        guard overlay != nil || popup != nil else { return }

        // Elegant fade-out animation
        UIView.animate(withDuration: 0.2, animations: {
            popup?.alpha = 0
            overlay?.alpha = 0
            popup?.transform = CGAffineTransform(scaleX: 0.9, y: 0.9)
        }) { _ in
            overlay?.removeFromSuperview()
            popup?.removeFromSuperview()
        }
    }

    @objc private func handleShowQualitySubmenu() {
        dismissSettingsPopup()
        showSubmenu(type: .quality)
    }

    @objc private func handleShowSpeedSubmenu() {
        dismissSettingsPopup()
        showSubmenu(type: .speed)
    }

    private enum SubmenuType {
        case quality
        case speed
    }

    private func showSubmenu(type: SubmenuType) {
        var yOffset: CGFloat = 8
        let menuWidth: CGFloat = 200

        // Create popup with solid background (simpler and more reliable for touch)
        let popup = UIView()
        popup.backgroundColor = UIColor(white: 0.18, alpha: 0.97)  // Dark semi-transparent
        popup.layer.cornerRadius = 13
        popup.clipsToBounds = true

        // Add subtle border and shadow for depth
        popup.layer.borderColor = UIColor.white.withAlphaComponent(0.15).cgColor
        popup.layer.borderWidth = 0.5
        popup.layer.shadowColor = UIColor.black.cgColor
        popup.layer.shadowOffset = CGSize(width: 0, height: 4)
        popup.layer.shadowRadius = 12
        popup.layer.shadowOpacity = 0.4

        switch type {
        case .quality:
            for (index, quality) in availableQualities.enumerated() {
                let label = quality.label
                let isSelected = (label == currentQuality)
                let button = createCompactMenuButton(title: label, isSelected: isSelected, tag: 1000 + index, action: #selector(handlePopupQualityTap(_:)))
                button.frame = CGRect(x: 8, y: yOffset, width: menuWidth - 16, height: 36)
                popup.addSubview(button)
                yOffset += 36
            }

        case .speed:
            for (index, rate) in availablePlaybackRates.enumerated() {
                let isSelected = (rate == currentPlaybackRate)
                let button = createCompactMenuButton(title: formattedRate(rate), isSelected: isSelected, tag: 2000 + index, action: #selector(handlePopupSpeedTap(_:)))
                button.frame = CGRect(x: 8, y: yOffset, width: menuWidth - 16, height: 36)
                popup.addSubview(button)
                yOffset += 36
            }
        }

        yOffset += 8
        popup.frame = CGRect(x: 0, y: 0, width: menuWidth, height: yOffset)

        // Position
        let settingsFrame = settingsButton.convert(settingsButton.bounds, to: view)
        var popupX: CGFloat
        var popupY: CGFloat

        if isFullscreen {
            popupX = settingsFrame.minX - menuWidth - 16
            popupY = max(20, min(settingsFrame.midY - popup.frame.height / 2, view.bounds.height - popup.frame.height - 20))
        } else {
            popupX = max(16, min(settingsFrame.midX - menuWidth / 2, view.bounds.width - menuWidth - 16))
            popupY = settingsFrame.minY - popup.frame.height - 12
        }
        popup.frame.origin = CGPoint(x: popupX, y: popupY)

        // Overlay
        let overlay = UIButton(type: .custom)
        overlay.frame = view.bounds
        overlay.backgroundColor = UIColor.black.withAlphaComponent(0.3)
        overlay.tag = 9999
        overlay.addTarget(self, action: #selector(dismissSettingsPopup), for: .touchUpInside)

        // Add overlay and popup
        view.addSubview(overlay)
        view.addSubview(popup)
        popup.tag = 9998

        // Animation
        let initialTransform: CGAffineTransform
        if isFullscreen {
            initialTransform = CGAffineTransform(scaleX: 0.8, y: 0.8).concatenating(CGAffineTransform(translationX: 40, y: 0))
        } else {
            initialTransform = CGAffineTransform(scaleX: 0.8, y: 0.8).concatenating(CGAffineTransform(translationX: 0, y: 20))
        }
        popup.transform = initialTransform
        popup.alpha = 0
        overlay.alpha = 0

        UIView.animate(withDuration: 0.3, delay: 0, usingSpringWithDamping: 0.8, initialSpringVelocity: 0.5, options: [.curveEaseOut]) {
            popup.transform = .identity
            popup.alpha = 1
            overlay.alpha = 1
        }
    }

    @objc private func handlePopupQualityTap(_ sender: UIButton) {
        let index = sender.tag - 1000
        if index >= 0 && index < availableQualities.count {
            let label = availableQualities[index].label
            handleQualitySelection(label: label)
        }
        dismissSettingsPopup()
    }

    @objc private func handlePopupSpeedTap(_ sender: UIButton) {
        let index = sender.tag - 2000
        if index >= 0 && index < availablePlaybackRates.count {
            let rate = availablePlaybackRates[index]
            handleSpeedSelection(rate: rate)
        }
        dismissSettingsPopup()
    }

    private func enterFullscreen() {
        guard !isFullscreen, let windowScene = view.window?.windowScene else { return }

        isFullscreen = true

        // Save original state
        originalSuperview = view.superview
        originalFrame = view.frame
        originalTransform = view.transform
        originalAutoresizingMask = view.autoresizingMask

        // Create dedicated fullscreen window (like MediaPreview)
        let window = UIWindow(windowScene: windowScene)
        window.windowLevel = .statusBar + 1  // Above status bar but don't affect main app
        window.backgroundColor = .black

        // Create fullscreen view controller with status bar hidden
        let viewController = FullscreenPlayerViewController()
        viewController.playerView = view
        // Set callback to update player layers when ViewController layouts
        viewController.onLayoutChanged = { [weak self] in
            guard let self = self else { return }
            CATransaction.begin()
            CATransaction.setDisableActions(true)
            self.playerLayer.frame = self.view.bounds
            CATransaction.commit()
            self.layoutOverlay()
        }
        window.rootViewController = viewController

        // Keep references
        fullscreenWindow = window
        fullscreenViewController = viewController

        // Remove from current parent
        view.removeFromSuperview()

        // Show fullscreen window
        window.makeKeyAndVisible()

        // Rotate the view content to landscape using transform
        let screenBounds = UIScreen.main.bounds
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }

            // Rotate view 90 degrees
            self.view.transform = CGAffineTransform(rotationAngle: .pi / 2)
            // Swap width and height to fit landscape
            self.view.frame = CGRect(x: 0, y: 0, width: screenBounds.height, height: screenBounds.width)

            // Update layers
            CATransaction.begin()
            CATransaction.setDisableActions(true)
            self.playerLayer.frame = self.view.bounds
            CATransaction.commit()
            self.layoutOverlay()
        }

        // Update UI
        fullscreenButton.setImage(LxIcon.image(named: "icon_fullscreen_exit"), for: .normal)
        updateCloseButtonVisibility()
        send(.fullscreenChange(fullScreen: true, direction: "horizontal"))

        os_log("MediaPlayer entered fullscreen", log: OSLog(subsystem: "LingXia", category: "Media"), type: .info)
    }

    private func exitFullscreen() {
        guard isFullscreen,
              let originalSuperview = originalSuperview else {
            os_log("MediaPlayer exitFullscreen failed: no original superview", log: OSLog(subsystem: "LingXia", category: "Media"), type: .error)
            return
        }

        os_log("MediaPlayer exitFullscreen: originalFrame=%{public}@", log: OSLog(subsystem: "LingXia", category: "Media"), type: .info, NSCoder.string(for: originalFrame))

        isFullscreen = false
        isTransitioningFullscreen = true  // Block external updates during transition

        // Save original window
        let originalWindow = originalSuperview.window

        // Remove from fullscreen window without animation
        view.removeFromSuperview()

        // Restore all original properties BEFORE adding back to parent
        // This prevents any intermediate layout/animation
        view.transform = originalTransform
        view.autoresizingMask = originalAutoresizingMask
        view.frame = originalFrame

        // Disable animations during restoration
        CATransaction.begin()
        CATransaction.setDisableActions(true)

        // Add back to original parent
        originalSuperview.addSubview(view)

        // Force frame to stay at original position
        view.frame = originalFrame

        CATransaction.commit()

        // Update player layers immediately
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        playerLayer.frame = view.bounds
        CATransaction.commit()
        layoutOverlay()

        // Clean up fullscreen window AFTER view is restored
        // Just hide the window, system will automatically restore the original key window
        fullscreenWindow?.isHidden = true
        fullscreenWindow?.rootViewController = nil
        fullscreenWindow = nil
        fullscreenViewController = nil

        // No need to explicitly call makeKey or rotate device
        // Transform is already restored to identity in the restoration code above

        // Update UI
        fullscreenButton.setImage(LxIcon.image(named: "icon_fullscreen_enter"), for: .normal)
        updateCloseButtonVisibility()
        send(.fullscreenChange(fullScreen: false, direction: "vertical"))

        os_log("MediaPlayer exited fullscreen", log: OSLog(subsystem: "LingXia", category: "Media"), type: .info)

        // Clear transition flag after a delay to allow JS side to settle
        // This prevents the JS side's component.update from overriding our restored frame
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { [weak self] in
            self?.isTransitioningFullscreen = false
            os_log("MediaPlayer fullscreen transition complete", log: OSLog(subsystem: "LingXia", category: "Media"), type: .info)
        }
    }

    @objc private func handleSliderTouchDown() {
        isScrubbing = true
        suppressWaitingUntil = .greatestFiniteMagnitude
        wasPlayingBeforeScrub = player?.timeControlStatus == .playing
        player?.pause()
        controlsHideWorkItem?.cancel()
    }

    @objc private func handleSliderChange() {
        controlsHideWorkItem?.cancel()
        
        // Real-time scrubbing - seek while dragging with throttling
        guard isScrubbing,
              let player = player,
              let durationSeconds = player.currentItem?.duration.seconds,
              durationSeconds.isFinite && durationSeconds > 0 else { return }
        
        let now = CACurrentMediaTime()
        // Throttle seeks to every 100ms to avoid overwhelming AVPlayer
        guard now - lastScrubTime > 0.1 else { return }
        lastScrubTime = now
        
        let target = Double(progressSlider.value) * durationSeconds
        let time = CMTime(seconds: target, preferredTimescale: 600)
        // Use toleranceBefore/After for faster scrubbing (not frame-accurate)
        player.seek(to: time, toleranceBefore: CMTime(seconds: 0.5, preferredTimescale: 600),
                    toleranceAfter: CMTime(seconds: 0.5, preferredTimescale: 600))
    }

    @objc private func handleSliderTouchUp() {
        guard isScrubbing else { return }
        isScrubbing = false
        suppressWaitingUntil = CACurrentMediaTime() + 1.5
        
        guard let player = player,
              let durationSeconds = player.currentItem?.duration.seconds,
              durationSeconds.isFinite && durationSeconds > 0 else { return }
        
        let target = Double(progressSlider.value) * durationSeconds
        
        // Final precise seek on release
        seek(to: target)
        
        // Resume playback if was playing before scrub
        if wasPlayingBeforeScrub {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in
                self?.player?.play()
            }
        }
        showControlsTemporarily()
    }
    
    // MARK: - Direct Slider Scrub Callbacks (from DebugSlider pan gesture)
    
    private func handleSliderScrubStart() {
        let status = player?.timeControlStatus
        wasPlayingBeforeScrub = status == .playing
        isScrubbing = true
        suppressWaitingUntil = .greatestFiniteMagnitude
        player?.pause()
        controlsHideWorkItem?.cancel()
    }
    
    private func handleSliderScrub(value: Float) {
        guard let player = player,
              let durationSeconds = player.currentItem?.duration.seconds,
              durationSeconds.isFinite && durationSeconds > 0 else { return }
        
        let target = Double(value) * durationSeconds
        let time = CMTime(seconds: target, preferredTimescale: 600)
        // Use toleranceBefore/After for faster scrubbing
        player.seek(to: time, toleranceBefore: CMTime(seconds: 0.5, preferredTimescale: 600),
                    toleranceAfter: CMTime(seconds: 0.5, preferredTimescale: 600))
    }
    
    private func handleSliderSeekComplete(value: Float) {
        guard let player = player,
              let durationSeconds = player.currentItem?.duration.seconds,
              durationSeconds.isFinite && durationSeconds > 0 else { return }
        
        let target = Double(value) * durationSeconds
        // Final precise seek
        seek(to: target)
    }
    
    private func handleSliderScrubEnd() {
        isScrubbing = false
        suppressWaitingUntil = CACurrentMediaTime() + 1.5
        
        // Resume playback after scrub/seek (user expects video to play after seeking)
        // timeControlStatusObserver will handle:
        // - .waitingToPlayAtSpecifiedRate -> show loading, send waiting event
        // - .playing -> hide loading, update UI
        player?.play()
        
        // Hide controls after delay
        showControlsTemporarily()
    }

    @objc private func handleVolumeTap() {
        guard let player = player else { return }

        // Toggle mute state
        let wasMuted = player.isMuted
        player.isMuted = !wasMuted

        // Update UI based on mute state
        if player.isMuted {
            updateVolumeIcon(volume: 0)
        } else {
            // Ensure volume is reasonable when unmuting
            if player.volume < 0.1 {
                player.volume = 0.7
                volumeSlider.value = 0.7
            }
            updateVolumeIcon(volume: player.volume)
        }

        send(.volumeChange(volume: Double(player.volume)))
        showControlsTemporarily()
    }

    @objc private func handleVolumeChange(sender: UISlider) {
        let vol = sender.value
        player?.volume = vol
        // Unmute when user adjusts volume slider
        if vol > 0 {
            player?.isMuted = false
        }
        updateVolumeIcon(volume: vol)
        send(.volumeChange(volume: Double(vol)))
    }

    private func updateVolumeIcon(volume: Float) {
        let iconName = volume == 0 ? "icon_volume_off" : "icon_volume_on"
        volumeButton.setImage(LxIcon.image(named: iconName), for: .normal)
    }

}

// MARK: - Fullscreen ViewController
private final class FullscreenPlayerViewController: UIViewController {
    weak var playerView: UIView?
    var onLayoutChanged: (() -> Void)?

    override var prefersStatusBarHidden: Bool { true }
    override var preferredStatusBarStyle: UIStatusBarStyle { .lightContent }
    override var supportedInterfaceOrientations: UIInterfaceOrientationMask {
        .portrait
    }
    override var shouldAutorotate: Bool { false }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black

        if let playerView = playerView {
            playerView.translatesAutoresizingMaskIntoConstraints = true
            playerView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            playerView.frame = view.bounds
            view.addSubview(playerView)
        }
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        playerView?.frame = view.bounds
        onLayoutChanged?()
    }
}

#endif
