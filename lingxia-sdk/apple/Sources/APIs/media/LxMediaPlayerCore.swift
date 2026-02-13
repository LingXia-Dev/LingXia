import Foundation
import AVFoundation
import QuartzCore

@MainActor
public final class LxMediaPlayerCore: NSObject {
    private(set) var player: AVPlayer?
    private(set) var playerItem: AVPlayerItem?
    private var timeObserver: Any?
    private var statusObserver: NSKeyValueObservation?
    private var timeControlStatusObserver: NSKeyValueObservation?
    private var loadingSequence: UInt64 = 0
    private var currentLoadingURL: URL?

    // MARK: - Playback state

    private(set) var isPlaying = false
    private(set) var isPausedByUser = false
    private(set) var urlHasEnded = false
    private var loopEnabled = false
    private var currentPlaybackRate: Double = 1.0
    private var availableQualities: [LxMediaQuality] = []
    private var currentQuality: String?
    private var availablePlaybackRates: [Double] = []
    private var pendingRestoreAfterLoad: (time: CMTime, play: Bool)?
    private var overrideDurationSeconds: Double?
    private var hasExternalDurationOverride = false
    private var suppressWaitingUntil: CFTimeInterval = 0
    private var suppressTimeUpdateUntil: CFTimeInterval = 0
    private var pendingSeekTargetSeconds: Double?
    private var pendingSeekDeadline: CFTimeInterval = 0
    private var pendingPlayingEvent = false

    // MARK: - Event callback

    var onEvent: ((LxMediaEvent) -> Void)?

    var onPlaybackStateChange: ((_ isPlaying: Bool, _ isBuffering: Bool) -> Void)?

    var onReadyToPlay: (() -> Void)?

    var onError: ((_ code: String, _ message: String) -> Void)?

    var onEnded: (() -> Void)?

    var onTimeUpdate: ((_ currentTime: Double, _ duration: Double) -> Void)?

    // MARK: - Public API

    public override init() {
        super.init()
    }

    deinit {
        NotificationCenter.default.removeObserver(self)
    }

    private(set) lazy var playerLayer: AVPlayerLayer = {
        let layer = AVPlayerLayer()
        layer.videoGravity = .resizeAspectFill
        return layer
    }()

    func effectiveDurationSeconds() -> Double? {
        if let item = player?.currentItem {
            let d = item.duration.seconds
            if d.isFinite, d > 0 { return d }
        }
        if let override = overrideDurationSeconds, override > 0 { return override }
        return nil
    }

    var currentTimeSeconds: Double {
        player?.currentTime().seconds ?? 0
    }

    func setExternalDurationSeconds(_ seconds: Double?) {
        if let seconds, seconds.isFinite, seconds > 0 {
            overrideDurationSeconds = seconds
            hasExternalDurationOverride = true
        } else {
            overrideDurationSeconds = nil
            hasExternalDurationOverride = false
        }
    }

    func setVideoGravity(_ gravity: AVLayerVideoGravity) {
        playerLayer.videoGravity = gravity
    }

    // MARK: - Load

    func loadVideo(url: URL) {
        loadingSequence &+= 1
        let seq = loadingSequence
        currentLoadingURL = url
        urlHasEnded = false
        suppressWaitingUntil = 0
        suppressTimeUpdateUntil = 0
        pendingSeekTargetSeconds = nil
        pendingSeekDeadline = 0
        pendingPlayingEvent = false

        timeObserver.flatMap { player?.removeTimeObserver($0) }
        timeObserver = nil
        statusObserver?.invalidate()
        statusObserver = nil
        timeControlStatusObserver?.invalidate()
        timeControlStatusObserver = nil

        let item = AVPlayerItem(url: url)
        item.audioTimePitchAlgorithm = .timeDomain
        item.preferredForwardBufferDuration = 2
        self.playerItem = item

        let activePlayer: AVPlayer
        if let existing = player {
            existing.replaceCurrentItem(with: item)
            activePlayer = existing
        } else {
            activePlayer = AVPlayer(playerItem: item)
            player = activePlayer
        }

        activePlayer.actionAtItemEnd = .pause
        activePlayer.rate = 0
        activePlayer.automaticallyWaitsToMinimizeStalling = true
        playerLayer.player = activePlayer

        statusObserver = item.observe(\.status, options: [.new]) { [weak self] item, _ in
            Task { @MainActor [weak self] in
                guard let self, seq == self.loadingSequence else { return }
                switch item.status {
                case .readyToPlay:
                    let duration = item.duration.seconds
                    let size = item.presentationSize
                    if duration.isFinite && size.width > 0 {
                        self.send(.loadedMetadata(width: size.width, height: size.height, duration: duration))
                    }
                    if let pending = self.pendingRestoreAfterLoad {
                        self.pendingRestoreAfterLoad = nil
                        self.player?.seek(to: pending.time, toleranceBefore: .zero, toleranceAfter: .zero) { [weak self] _ in
                            guard pending.play else { return }
                            Task { @MainActor [weak self] in
                                self?.play()
                            }
                        }
                    }
                    self.onReadyToPlay?()
                case .failed:
                    let msg = item.error?.localizedDescription ?? "unknown error"
                    self.send(.error(code: "load_failed", message: msg))
                    self.onError?("load_failed", msg)
                default:
                    break
                }
            }
        }

        timeControlStatusObserver = activePlayer.observe(\.timeControlStatus, options: [.new, .old]) { [weak self] player, _ in
            Task { @MainActor [weak self] in
                guard let self, seq == self.loadingSequence else { return }
                switch player.timeControlStatus {
                case .playing:
                    let wasPlaying = self.isPlaying
                    self.isPlaying = true
                    self.suppressWaitingUntil = 0
                    if self.pendingPlayingEvent || !wasPlaying {
                        self.pendingPlayingEvent = false
                        self.send(.playing)
                    }
                    self.onPlaybackStateChange?(true, false)
                case .paused:
                    self.isPlaying = false
                    self.pendingPlayingEvent = false
                    self.onPlaybackStateChange?(false, false)
                case .waitingToPlayAtSpecifiedRate:
                    if !self.isPausedByUser && CACurrentMediaTime() >= self.suppressWaitingUntil {
                        self.isPlaying = false
                        self.pendingPlayingEvent = true
                        self.onPlaybackStateChange?(false, true)
                        self.send(.waiting)
                    }
                @unknown default:
                    break
                }
            }
        }

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
            let error = (notification.userInfo?[AVPlayerItemFailedToPlayToEndTimeErrorKey] as? NSError)?.localizedDescription ?? "unknown"
            Task { @MainActor [weak self] in
                guard let self, seq == self.loadingSequence else { return }
                self.send(.error(code: "play_failed", message: error))
                self.onError?("play_failed", error)
            }
        }

        let interval = CMTime(seconds: 0.25, preferredTimescale: 600)
        timeObserver = activePlayer.addPeriodicTimeObserver(forInterval: interval, queue: .main) { [weak self] time in
            Task { @MainActor [weak self] in
                guard let self, seq == self.loadingSequence else { return }
                let current = time.seconds
                guard current.isFinite else { return }
                if CACurrentMediaTime() < self.suppressTimeUpdateUntil {
                    return
                }
                if let pendingSeek = self.pendingSeekTargetSeconds {
                    if current + 0.2 < pendingSeek {
                        if CACurrentMediaTime() < self.pendingSeekDeadline {
                            return
                        }
                        self.pendingSeekTargetSeconds = nil
                        self.pendingSeekDeadline = 0
                    } else if abs(current - pendingSeek) <= 0.3 || current > pendingSeek {
                        self.pendingSeekTargetSeconds = nil
                        self.pendingSeekDeadline = 0
                    }
                }
                let duration = self.effectiveDurationSeconds() ?? 0
                self.send(.timeUpdate(currentTime: current, duration: duration))
                self.onTimeUpdate?(current, duration)
            }
        }
    }

    // MARK: - Playback controls

    func play() {
        let wasEnded = urlHasEnded
        urlHasEnded = false
        isPausedByUser = false
        suppressWaitingUntil = 0
        pendingPlayingEvent = true
        send(.raw(name: "playrequest", data: [:]))

        guard player != nil else { return }

        if wasEnded {
            player?.seek(to: .zero, toleranceBefore: .zero, toleranceAfter: .zero) { [weak self] _ in
                Task { @MainActor [weak self] in
                    guard let self else { return }
                    self.player?.playImmediately(atRate: Float(self.currentPlaybackRate))
                }
            }
        } else {
            player?.playImmediately(atRate: Float(currentPlaybackRate))
        }

        send(.play)
    }

    func pause() {
        isPausedByUser = true
        suppressWaitingUntil = CACurrentMediaTime() + 0.6
        pendingPlayingEvent = false
        player?.pause()
        isPlaying = false
        onPlaybackStateChange?(false, false)
        send(.pause)
    }

    func stop() {
        isPausedByUser = true
        suppressWaitingUntil = CACurrentMediaTime() + 0.8
        suppressTimeUpdateUntil = CACurrentMediaTime() + 0.25
        pendingSeekTargetSeconds = nil
        pendingPlayingEvent = false
        player?.pause()
        player?.seek(to: .zero, toleranceBefore: .zero, toleranceAfter: .zero)
        isPlaying = false
        urlHasEnded = false
        onPlaybackStateChange?(false, false)
        if let duration = effectiveDurationSeconds(), duration > 0 {
            onTimeUpdate?(0, duration)
        }
        send(.stop)
    }

    func seek(to seconds: Double) {
        guard seconds.isFinite else { return }
        guard let player else { return }
        urlHasEnded = false
        let clamped: Double = {
            let target = max(0, seconds)
            if let duration = effectiveDurationSeconds(), duration.isFinite, duration > 0 {
                return min(target, duration)
            }
            return target
        }()
        suppressWaitingUntil = CACurrentMediaTime() + 1.2
        suppressTimeUpdateUntil = CACurrentMediaTime() + 0.2
        pendingSeekTargetSeconds = clamped
        pendingSeekDeadline = CACurrentMediaTime() + 2.0
        send(.raw(name: "seeking", data: ["time": clamped]))

        if let duration = effectiveDurationSeconds(), duration > 0 {
            onTimeUpdate?(clamped, duration)
        }

        let time = CMTime(seconds: clamped, preferredTimescale: 600)
        player.seek(to: time, toleranceBefore: .zero, toleranceAfter: .zero) { [weak self] finished in
            Task { @MainActor [weak self] in
                guard let self, finished else { return }
                let actual = self.player?.currentTime().seconds ?? clamped
                let resolved = actual.isFinite && actual >= 0 ? actual : clamped
                self.pendingSeekTargetSeconds = nil
                self.pendingSeekDeadline = 0
                if let duration = self.effectiveDurationSeconds(), duration > 0 {
                    self.onTimeUpdate?(resolved, duration)
                }
                self.send(.seeked(time: resolved))
            }
        }
    }

    func setVolume(_ volume: Double) {
        player?.volume = Float(max(0, min(1, volume)))
        send(.volumeChange(volume: volume))
    }

    func setMuted(_ muted: Bool) {
        player?.isMuted = muted
        send(.volumeChange(volume: muted ? 0 : Double(player?.volume ?? 1)))
    }

    func setPlaybackRate(_ rate: Double) {
        currentPlaybackRate = rate
        if isPlaying {
            player?.rate = Float(rate)
        }
        send(.rateChange(rate: rate))
    }

    func setLoop(_ loop: Bool) {
        loopEnabled = loop
    }

    // MARK: - Quality switching

    func setQualities(_ qualities: [LxMediaQuality]) {
        availableQualities = qualities
        if currentQuality == nil, let first = qualities.first {
            currentQuality = first.label
        }
    }

    func switchQuality(to label: String) {
        guard let quality = availableQualities.first(where: { $0.label == label }),
              let url = quality.url else { return }

        let currentTime = player?.currentTime() ?? .zero
        let wasPlaying = isPlaying

        currentQuality = label
        pendingRestoreAfterLoad = (time: currentTime, play: wasPlaying)
        loadVideo(url: url)
        send(.qualityChange(quality: label, url: url.absoluteString))
    }

    // MARK: - Command dispatch

    func handle(command: LxMediaCommand) {
        switch command {
        case .play: play()
        case .pause: pause()
        case .stop: stop()
        case .seek(let time): seek(to: time)
        case .setVolume(let vol): setVolume(vol)
        case .setMuted(let m): setMuted(m)
        case .setPlaybackRate(let r): setPlaybackRate(r)
        case .enterFullscreen, .exitFullscreen:
            break // Handled by UI layer
        }
    }

    // MARK: - Private

    @objc private func videoDidEnd() {
        Task { @MainActor in
            if loopEnabled {
                urlHasEnded = false
                let rate = Float(currentPlaybackRate)
                player?.seek(to: .zero, toleranceBefore: .zero, toleranceAfter: .zero) { [weak self] finished in
                    Task { @MainActor [weak self] in
                        guard let self, finished, self.loopEnabled else { return }
                        self.player?.playImmediately(atRate: rate)
                    }
                }
                send(.ended)
                return
            }

            isPlaying = false
            urlHasEnded = true
            send(.ended)
            onEnded?()
        }
    }

    private func send(_ event: LxMediaEvent) {
        onEvent?(event)
    }

    func cleanup() {
        timeObserver.flatMap { player?.removeTimeObserver($0) }
        timeObserver = nil
        statusObserver?.invalidate()
        statusObserver = nil
        timeControlStatusObserver?.invalidate()
        timeControlStatusObserver = nil
        NotificationCenter.default.removeObserver(self)
        player?.pause()
        player?.replaceCurrentItem(with: nil)
        player = nil
        playerItem = nil
    }
}
