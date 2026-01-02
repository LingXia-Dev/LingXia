import Foundation
import OSLog
import AVFoundation
import UIKit

#if os(iOS)

@MainActor
final class StreamDecoderRegistry {
    static let shared = StreamDecoderRegistry()

    private let log = OSLog(subsystem: "LingXia", category: "StreamDecoder")
    private var sessions: [String: StreamDecoderSession] = [:]
    private var lastVideoConfigJson: [String: String] = [:]
    private var lastAudioConfigJson: [String: String] = [:]
    private var stopping: Set<String> = []

    func create(componentId: String) -> Bool {
        stopping.remove(componentId)
        guard let view = ComponentRouter.shared.componentView(componentId: componentId) else {
            os_log("Stream decoder create failed: component not found %{public}@", log: log, type: .error, componentId)
            return false
        }
        if let existing = sessions[componentId], existing.usesContainerView(view) {
            ComponentRouter.shared.setStreamDecoderActive(componentId: componentId, active: true)
            return true
        }
        if let existing = sessions[componentId] {
            existing.stop()
        }
        sessions[componentId] = StreamDecoderSession(componentId: componentId, containerView: view, log: log)
        ComponentRouter.shared.setStreamDecoderActive(componentId: componentId, active: true)
        return true
    }

    func configureVideo(componentId: String, configJson: String) -> Bool {
        stopping.remove(componentId)
        lastVideoConfigJson[componentId] = configJson
        guard let session = ensureSession(componentId: componentId, reason: "configureVideo") else {
            return false
        }
        session.configureVideo(configJson)
        return true
    }

    func configureAudio(componentId: String, configJson: String) -> Bool {
        stopping.remove(componentId)
        lastAudioConfigJson[componentId] = configJson
        guard let session = ensureSession(componentId: componentId, reason: "configureAudio") else {
            return false
        }
        session.configureAudio(configJson)
        return true
    }

    func pushVideo(componentId: String, data: Data, dtsMs: UInt32, ptsMs: UInt32, keyframe: Bool) -> Bool {
        stopping.remove(componentId)

        guard let session = ensureSession(componentId: componentId, reason: "pushVideo") else {
            return false
        }
        guard applyCachedConfigsIfNeeded(
            componentId: componentId,
            session: session,
            requireVideo: true,
            requireAudio: false
        ) else {
            return false
        }
        session.pushVideo(data: data, dtsMs: dtsMs, ptsMs: ptsMs, keyframe: keyframe)
        return true
    }

    func pushAudio(componentId: String, data: Data, dtsMs: UInt32, ptsMs: UInt32) -> Bool {
        stopping.remove(componentId)

        guard let session = ensureSession(componentId: componentId, reason: "pushAudio") else {
            return false
        }
        guard applyCachedConfigsIfNeeded(
            componentId: componentId,
            session: session,
            requireVideo: false,
            requireAudio: true
        ) else {
            return false
        }
        session.pushAudio(data: data, dtsMs: dtsMs, ptsMs: ptsMs)
        return true
    }

    func stop(componentId: String) -> Bool {
        stopping.insert(componentId)
        let session = sessions.removeValue(forKey: componentId)
        let shouldForget = ComponentRouter.shared.componentView(componentId: componentId) == nil
        if shouldForget {
            lastVideoConfigJson.removeValue(forKey: componentId)
            lastAudioConfigJson.removeValue(forKey: componentId)
            stopping.remove(componentId)
        }
        os_log(
            "Stream decoder stop for %{public}@ forget_cache=%{public}@",
            log: log,
            type: .info,
            componentId,
            String(shouldForget)
        )
        session?.stop()
        ComponentRouter.shared.setStreamDecoderActive(componentId: componentId, active: false)
        return true
    }

    func handleCommand(componentId: String, name: String, params: [String: Any]?) -> Bool {
        stopping.remove(componentId)
        guard let session = sessions[componentId] else {
            return false
        }
        return session.handleCommand(name: name, params: params)
    }

    func hasSession(componentId: String) -> Bool {
        return sessions[componentId] != nil
    }

    private func ensureSession(componentId: String, reason: StaticString) -> StreamDecoderSession? {
        if let session = sessions[componentId] { return session }
        os_log(
            "Stream decoder missing session for %{public}@ (%{public}@), attempting recreate",
            log: log,
            type: .error,
            componentId,
            String(describing: reason)
        )
        guard create(componentId: componentId), let session = sessions[componentId] else {
            os_log(
                "Stream decoder recreate failed for %{public}@ (%{public}@)",
                log: log,
                type: .error,
                componentId,
                String(describing: reason)
            )
            return nil
        }
        return session
    }

    private func applyCachedConfigsIfNeeded(
        componentId: String,
        session: StreamDecoderSession,
        requireVideo: Bool,
        requireAudio: Bool
    ) -> Bool {
        if requireVideo {
            guard let cachedVideo = lastVideoConfigJson[componentId] else {
                os_log(
                    "Stream decoder missing cached video config for %{public}@",
                    log: log,
                    type: .error,
                    componentId
                )
                return false
            }
            session.configureVideo(cachedVideo)
        } else if let cachedVideo = lastVideoConfigJson[componentId] {
            session.configureVideo(cachedVideo)
        }

        if requireAudio {
            guard let cachedAudio = lastAudioConfigJson[componentId] else {
                os_log(
                    "Stream decoder missing cached audio config for %{public}@",
                    log: log,
                    type: .error,
                    componentId
                )
                return false
            }
            session.configureAudio(cachedAudio)
        } else if let cachedAudio = lastAudioConfigJson[componentId] {
            session.configureAudio(cachedAudio)
        }
        return true
    }
}

private final class StreamVideoLayerView: UIView {
    override class var layerClass: AnyClass {
        AVSampleBufferDisplayLayer.self
    }

    var displayLayer: AVSampleBufferDisplayLayer {
        layer as! AVSampleBufferDisplayLayer
    }
}

private final class StreamDecoderSession {
    private final class WeakRef<T: AnyObject> {
        weak var value: T?
        init(_ value: T) {
            self.value = value
        }
    }

    private let componentId: String
    private weak var containerView: UIView?
    private let log: OSLog

    private let renderSynchronizer = AVSampleBufferRenderSynchronizer()
    private let videoLayerView: StreamVideoLayerView
    private let audioRenderer = AVSampleBufferAudioRenderer()
    private let decodeQueue: DispatchQueue

    private var videoConfig: VideoConfig?
    private var audioConfig: AudioConfig?
    private var videoFormatDescription: CMFormatDescription?
    private var audioFormatDescription: CMAudioFormatDescription?
    private var isPlaying = false
    private var metadataNotified = false
    private var videoBaseTimeMs: UInt32?
    private var audioBaseTimeMs: UInt32?
    private var lastVideoPtsMs: UInt32?
    private var lastVideoDtsMs: UInt32?
    private var lastAudioPtsMs: UInt32?
    private var pcmAudioPtsMs: UInt32 = 0
    private var audioSessionConfigured = false
    private var audioEngine: AVAudioEngine?
    private var audioPlayer: AVAudioPlayerNode?
    private var pcmAudioFormat: AVAudioFormat?
    private var usePcmAudioEngine = false
    private var streamVolume: Float = 1.0
    private var streamMuted = false
    private var waitingForVideoKeyframe = false
    private var resumeAfterVideoKeyframe = false
    private var playRequested = false
    private var gateAudioUntilVideo = false
    private var suppressedLayers: [WeakRef<CALayer>] = []
    private var suppressedViews: [WeakRef<UIView>] = []
    private var appObservers: [NSObjectProtocol] = []
    private var wasPlayingBeforeBackground = false
    private var pendingVideoKeyframe: (data: Data, dtsMs: UInt32, ptsMs: UInt32)?

    @MainActor
    init(componentId: String, containerView: UIView, log: OSLog) {
        self.componentId = componentId
        self.containerView = containerView
        self.log = log
        self.decodeQueue = DispatchQueue(label: "LingXia.StreamDecoder.\(componentId)")
        self.videoLayerView = StreamVideoLayerView()

        setupLayers(on: containerView)
        renderSynchronizer.addRenderer(videoLayerView.displayLayer)
        renderSynchronizer.addRenderer(audioRenderer)
        renderSynchronizer.rate = 0.0

        let center = NotificationCenter.default
        appObservers.append(
            center.addObserver(forName: UIApplication.willResignActiveNotification, object: nil, queue: .main) { [weak self] _ in
                Task { @MainActor in
                    self?.handleAppWillResignActive()
                }
            }
        )
        appObservers.append(
            center.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil, queue: .main) { [weak self] _ in
                Task { @MainActor in
                    self?.handleAppDidBecomeActive()
                }
            }
        )
    }

    func usesContainerView(_ view: UIView) -> Bool {
        containerView === view
    }

    @MainActor
    func configureVideo(_ configJson: String) {
        guard let config = VideoConfig(json: configJson) else {
            emitError("invalid video config")
            return
        }
        let shouldHardReset = shouldHardResetVideoConfigChange(next: config)
        if shouldHardReset {
            resetStream(hard: true)
        }
        videoConfig = config
        
        updateCornerRadius()
        
        decodeQueue.async { [weak self] in
            self?.buildVideoFormat(config: config)
        }
        if !metadataNotified {
            metadataNotified = true
            ComponentRouter.shared.emitComponentEvent(
                componentId: componentId,
                event: "loadedmetadata",
                detail: ["width": config.width ?? 0, "height": config.height ?? 0, "duration": 0]
            )
        }
    }

    @MainActor
    func configureAudio(_ configJson: String) {
        guard let config = AudioConfig(json: configJson) else {
            emitError("invalid audio config")
            return
        }
        let shouldHardReset = shouldHardResetAudioConfigChange(next: config)
        if shouldHardReset {
            resetStream(hard: true)
        }
        let oldCodec = audioConfig?.codec
        audioConfig = config
        ensureAudioSession(sampleRate: config.sampleRate)
        if config.codec == "pcm_s16le" {
            setupPcmAudioEngine(sampleRate: config.sampleRate ?? 44100, channels: config.channels ?? 1)
        } else if oldCodec == "pcm_s16le" {
            stopPcmAudioEngine()
        }
        decodeQueue.async { [weak self] in
            self?.buildAudioFormat(config: config)
        }
    }

    func pushVideo(data: Data, dtsMs: UInt32, ptsMs: UInt32, keyframe: Bool) {
        decodeQueue.async { [weak self] in
            guard let self = self else { return }
            if self.waitingForVideoKeyframe {
                let inferredKeyframe = keyframe || self.inferKeyframe(data: data)
                if !inferredKeyframe {
                    return
                }
                if self.videoFormatDescription == nil {
                    self.pendingVideoKeyframe = (data: data, dtsMs: dtsMs, ptsMs: ptsMs)
                    return
                }
                self.waitingForVideoKeyframe = false
                if self.resumeAfterVideoKeyframe {
                    self.resumeAfterVideoKeyframe = false
                    if self.playRequested && self.renderSynchronizer.rate == 0.0 {
                        self.renderSynchronizer.rate = 1.0
                    }
                }
            }
            guard let format = self.videoFormatDescription else { return }
            let (normalizedDts, normalizedPts) = self.normalizeVideoTimes(dtsMs: dtsMs, ptsMs: ptsMs)
            let sampleData = self.convertVideoData(data)
            guard let sampleBuffer = self.makeSampleBuffer(
                data: sampleData,
                formatDescription: format,
                dtsMs: normalizedDts,
                ptsMs: normalizedPts
            ) else {
                os_log(
                    "makeSampleBuffer(video) failed codec=%{public}@ format=%{public}@ data_len=%{public}@ pts=%{public}@ dts=%{public}@",
                    log: self.log,
                    type: .error,
                    self.videoConfig?.codec ?? "unknown",
                    self.videoConfig?.format ?? "unknown",
                    String(sampleData.count),
                    String(normalizedPts),
                    String(normalizedDts)
                )
                return
            }
            self.videoLayerView.displayLayer.enqueue(sampleBuffer)
            if !self.isPlaying && self.playRequested {
                self.isPlaying = true
                self.gateAudioUntilVideo = false
                let componentId = self.componentId
                DispatchQueue.main.async {
                    ComponentRouter.shared.emitComponentEvent(
                        componentId: componentId,
                        event: "play",
                        detail: [:]
                    )
                }
            }
            if self.playRequested && self.renderSynchronizer.rate == 0.0 && !self.waitingForVideoKeyframe {
                self.renderSynchronizer.rate = 1.0
            }
        }
    }

    func pushAudio(data: Data, dtsMs: UInt32, ptsMs: UInt32) {
        decodeQueue.async { [weak self] in
            guard let self = self else { return }
            if self.gateAudioUntilVideo || self.waitingForVideoKeyframe {
                return
            }
            guard let format = self.audioFormatDescription else { return }
            if self.usePcmAudioEngine, let audioFormat = self.pcmAudioFormat {
                self.enqueuePcmAudio(data: data, format: audioFormat)
                return
            }
            let sampleBuffer: CMSampleBuffer?
            if self.audioConfig?.codec == "pcm_s16le" {
                let sampleRate = self.audioConfig?.sampleRate ?? 44100
                let channels = self.audioConfig?.channels ?? 2
                let bytesPerFrame = max(1, channels * 2)
                let sampleCount = max(1, data.count / bytesPerFrame)
                let durationMs = UInt32((sampleCount * 1000) / max(1, sampleRate))
                let ptsMs = self.pcmAudioPtsMs
                let (next, overflow) = self.pcmAudioPtsMs.addingReportingOverflow(durationMs)
                self.pcmAudioPtsMs = overflow ? 0 : next
                sampleBuffer = self.makeAudioSampleBuffer(
                    data: data,
                    formatDescription: format,
                    dtsMs: ptsMs,
                    ptsMs: ptsMs
                )
            } else {
                let normalizedDts = self.normalizeAudioTime(dtsMs: dtsMs)
                sampleBuffer = self.makeSampleBuffer(
                    data: data,
                    formatDescription: format,
                    dtsMs: normalizedDts,
                    ptsMs: normalizedDts
                )
            }
            guard let sampleBuffer else {
                os_log(
                    "makeSampleBuffer(audio) failed codec=%{public}@ data_len=%{public}@ dts=%{public}@ pts=%{public}@",
                    log: self.log,
                    type: .error,
                    self.audioConfig?.codec ?? "unknown",
                    String(data.count),
                    String(dtsMs),
                    String(ptsMs)
                )
                return
            }
            self.audioRenderer.enqueue(sampleBuffer)
            if self.renderSynchronizer.rate == 0.0
                && self.isPlaying
                && self.playRequested
                && !self.waitingForVideoKeyframe
                && !self.gateAudioUntilVideo
            {
                self.renderSynchronizer.rate = 1.0
            }
        }
    }

    @MainActor
    func handleCommand(name: String, params: [String: Any]?) -> Bool {
        switch name {
        case "play":
            playRequested = true
            if let view = containerView {
                suppressNativePlayback(in: view)
            }
            updateCornerRadius()
            if waitingForVideoKeyframe || !isPlaying {
                gateAudioUntilVideo = true
                resumeAfterVideoKeyframe = true
                renderSynchronizer.rate = 0.0
                ComponentRouter.shared.emitComponentEvent(componentId: componentId, event: "waiting", detail: [:])
            } else {
                renderSynchronizer.rate = 1.0
                ComponentRouter.shared.emitComponentEvent(componentId: componentId, event: "play", detail: [:])
            }
            return true
        case "pause":
            playRequested = false
            renderSynchronizer.rate = 0.0
            isPlaying = false
            ComponentRouter.shared.emitComponentEvent(componentId: componentId, event: "pause", detail: [:])
            return true
        case "stop":
            stopPlayback()
            ComponentRouter.shared.emitComponentEvent(componentId: componentId, event: "stop", detail: [:])
            return true
        case "resetStream":
            let hard = (params?["hard"] as? Bool) ?? false
            resetStream(hard: hard)
            return true
        case "setVolume":
            if let volume = params?["volume"] as? Double {
                setStreamVolume(Float(volume))
                ComponentRouter.shared.emitComponentEvent(
                    componentId: componentId,
                    event: "volumechange",
                    detail: ["volume": streamVolume]
                )
                return true
            }
            return false
        case "setMuted":
            if let muted = params?["muted"] as? Bool {
                streamMuted = muted
                applyStreamVolume()
                ComponentRouter.shared.emitComponentEvent(
                    componentId: componentId,
                    event: "volumechange",
                    detail: ["muted": muted, "volume": streamVolume]
                )
                return true
            }
            return false
        default:
            return false
        }
    }

    @MainActor
    private func stopPlayback() {
        renderSynchronizer.rate = 0.0
        isPlaying = false
        waitingForVideoKeyframe = false
        resumeAfterVideoKeyframe = false
        playRequested = false
        gateAudioUntilVideo = false
        pendingVideoKeyframe = nil

        videoBaseTimeMs = nil
        audioBaseTimeMs = nil
        lastVideoPtsMs = nil
        lastVideoDtsMs = nil
        lastAudioPtsMs = nil
        pcmAudioPtsMs = 0

        decodeQueue.async { [weak self] in
            self?.videoLayerView.displayLayer.flushAndRemoveImage()
            self?.audioRenderer.flush()
        }

        stopPcmAudioEngine()
        restoreNativePlayback()
    }

    @MainActor
    private func resetStream(hard: Bool) {
        let wasRunning = playRequested || renderSynchronizer.rate != 0.0 || isPlaying
        renderSynchronizer.rate = 0.0
        isPlaying = false

        if wasRunning { playRequested = true }

        waitingForVideoKeyframe = true
        resumeAfterVideoKeyframe = wasRunning
        gateAudioUntilVideo = wasRunning
        pendingVideoKeyframe = nil

        if wasRunning {
            ComponentRouter.shared.emitComponentEvent(componentId: componentId, event: "waiting", detail: [:])
        }

        videoBaseTimeMs = nil
        audioBaseTimeMs = nil
        lastVideoPtsMs = nil
        lastVideoDtsMs = nil
        lastAudioPtsMs = nil
        pcmAudioPtsMs = 0

        if hard {
            videoFormatDescription = nil
            audioFormatDescription = nil
            metadataNotified = false
        }

        decodeQueue.async { [weak self] in
            guard let self = self else { return }
            self.videoLayerView.displayLayer.flushAndRemoveImage()
            self.audioRenderer.flush()
        }

        if let view = containerView {
            suppressNativePlayback(in: view)
        }
    }

    private func shouldHardResetVideoConfigChange(next: VideoConfig) -> Bool {
        guard let current = videoConfig else { return false }
        if current.codec != next.codec { return true }
        if current.format != next.format { return true }
        if current.nalLengthSize != next.nalLengthSize { return true }
        if current.sps != next.sps { return true }
        if current.pps != next.pps { return true }
        if current.vps != next.vps { return true }
        if current.width != next.width { return true }
        if current.height != next.height { return true }
        return false
    }

    private func shouldHardResetAudioConfigChange(next: AudioConfig) -> Bool {
        guard let current = audioConfig else { return false }
        if current.codec != next.codec { return true }
        if current.sampleRate != next.sampleRate { return true }
        if current.channels != next.channels { return true }
        if current.aacIsAdts != next.aacIsAdts { return true }
        if current.audioSpecificConfig != next.audioSpecificConfig { return true }
        return false
    }

    @MainActor
    func stop() {
        stopPlayback()
        videoLayerView.removeFromSuperview()
        renderSynchronizer.removeRenderer(videoLayerView.displayLayer, at: .zero)
        renderSynchronizer.removeRenderer(audioRenderer, at: .zero)
        for token in appObservers {
            NotificationCenter.default.removeObserver(token)
        }
        appObservers.removeAll()
    }

    @MainActor
    private func setupLayers(on view: UIView) {
        suppressNativePlayback(in: view)
        videoLayerView.frame = view.bounds
        videoLayerView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        videoLayerView.displayLayer.videoGravity = .resizeAspect
        videoLayerView.displayLayer.needsDisplayOnBoundsChange = true
        
        view.insertSubview(videoLayerView, at: 0)
        
        updateCornerRadius()
    }
    
    @MainActor
    private func updateCornerRadius() {
        guard let view = containerView else { return }
        let radius = view.layer.cornerRadius

        videoLayerView.layer.cornerRadius = radius
        videoLayerView.layer.masksToBounds = radius > 0
        videoLayerView.clipsToBounds = radius > 0
        videoLayerView.backgroundColor = .clear
        
        videoLayerView.displayLayer.frame = videoLayerView.bounds
        videoLayerView.displayLayer.cornerRadius = radius
        videoLayerView.displayLayer.masksToBounds = radius > 0
        
        videoLayerView.setNeedsLayout()
        videoLayerView.layoutIfNeeded()
    }

    private func buildVideoFormat(config: VideoConfig) {
        let nalLength = max(1, min(4, config.nalLengthSize ?? 4))
        switch config.codec {
        case "h265":
            guard #available(iOS 11.0, *) else {
                emitError("H265 not supported")
                return
            }
            guard let format = buildHevcFormat(config: config, nalLength: nalLength) else {
                emitError("H265 format build failed")
                return
            }
            videoFormatDescription = format
        default:
            guard let format = buildH264Format(config: config, nalLength: nalLength) else {
                emitError("H264 format build failed")
                return
            }
            videoFormatDescription = format
        }

        if waitingForVideoKeyframe,
           let pending = pendingVideoKeyframe,
           let format = videoFormatDescription
        {
            pendingVideoKeyframe = nil
            waitingForVideoKeyframe = false
            if resumeAfterVideoKeyframe {
                resumeAfterVideoKeyframe = false
                if playRequested && renderSynchronizer.rate == 0.0 {
                    renderSynchronizer.rate = 1.0
                }
            }
            let (normalizedDts, normalizedPts) = normalizeVideoTimes(dtsMs: pending.dtsMs, ptsMs: pending.ptsMs)
            let sampleData = convertVideoData(pending.data)
            guard let sampleBuffer = makeSampleBuffer(
                data: sampleData,
                formatDescription: format,
                dtsMs: normalizedDts,
                ptsMs: normalizedPts
            ) else { return }
            videoLayerView.displayLayer.enqueue(sampleBuffer)
            if !isPlaying && playRequested {
                isPlaying = true
                gateAudioUntilVideo = false
                let componentId = self.componentId
                DispatchQueue.main.async {
                    ComponentRouter.shared.emitComponentEvent(
                        componentId: componentId,
                        event: "play",
                        detail: [:]
                    )
                }
            }
            if playRequested && renderSynchronizer.rate == 0.0 {
                renderSynchronizer.rate = 1.0
            }
        }
    }

    private func sanitizeParameterSet(_ data: Data, codec: String, expectedNalType: Int) -> Data {
        if data.isEmpty { return data }

        var start = 0
        var end = data.count

        while end > start, data[end - 1] == 0 {
            end -= 1
        }

        func nalType(of bytes: Data, at offset: Int) -> Int? {
            guard offset < bytes.count else { return nil }
            let first = bytes[offset]
            if codec == "h265" || codec == "hevc" {
                return Int((first >> 1) & 0x3F)
            }
            return Int(first & 0x1F)
        }

        // Strip AnnexB start codes.
        if end - start >= 4,
           data[start] == 0, data[start + 1] == 0, data[start + 2] == 0, data[start + 3] == 1
        {
            start += 4
        } else if end - start >= 3,
                  data[start] == 0, data[start + 1] == 0, data[start + 2] == 1
        {
            start += 3
        }

        // Strip a single length prefix if the buffer looks like <len><nal>.
        for lengthSize in [4, 3, 2, 1] {
            if end - start <= lengthSize { continue }
            var nalLen = 0
            for i in 0..<lengthSize {
                nalLen = (nalLen << 8) | Int(data[start + i])
            }
            if nalLen != (end - start - lengthSize) { continue }
            let candidateStart = start + lengthSize
            if nalType(of: data, at: candidateStart) == expectedNalType {
                start = candidateStart
                break
            }
        }

        if start >= end { return Data() }
        return data.subdata(in: start..<end)
    }

    private func bytesPrefixHex(_ data: Data, count: Int) -> String {
        if data.isEmpty { return "" }
        let n = min(count, data.count)
        return data.prefix(n).map { String(format: "%02x", $0) }.joined()
    }

    private func logFormatFailure(_ label: String, status: OSStatus, sets: [(String, Data)]) {
        let details = sets
            .map { "\($0.0)=\($0.1.count)(\(bytesPrefixHex($0.1, count: 8)))" }
            .joined(separator: " ")
        os_log(
            "%{public}@ failed status=%d %{public}@",
            log: log,
            type: .error,
            label,
            status,
            details
        )
    }

    private func buildH264Format(config: VideoConfig, nalLength: Int) -> CMFormatDescription? {
        let sps = sanitizeParameterSet(config.sps, codec: "h264", expectedNalType: 7)
        let pps = sanitizeParameterSet(config.pps, codec: "h264", expectedNalType: 8)
        guard !sps.isEmpty, !pps.isEmpty else { return nil }
        var format: CMFormatDescription?
        sps.withUnsafeBytes { spsPtr in
            pps.withUnsafeBytes { ppsPtr in
                guard let spsBase = spsPtr.bindMemory(to: UInt8.self).baseAddress,
                      let ppsBase = ppsPtr.bindMemory(to: UInt8.self).baseAddress else {
                    return
                }
                var parameterSetPointers: [UnsafePointer<UInt8>] = [spsBase, ppsBase]
                var parameterSetSizes = [sps.count, pps.count]
                let status = CMVideoFormatDescriptionCreateFromH264ParameterSets(
                    allocator: kCFAllocatorDefault,
                    parameterSetCount: 2,
                    parameterSetPointers: &parameterSetPointers,
                    parameterSetSizes: &parameterSetSizes,
                    nalUnitHeaderLength: Int32(nalLength),
                    formatDescriptionOut: &format
                )
                if status != noErr {
                    logFormatFailure(
                        "buildH264Format",
                        status: status,
                        sets: [("sps", sps), ("pps", pps)]
                    )
                }
            }
        }
        return format
    }

    @available(iOS 11.0, *)
    private func buildHevcFormat(config: VideoConfig, nalLength: Int) -> CMFormatDescription? {
        let vps = sanitizeParameterSet(config.vps, codec: "h265", expectedNalType: 32)
        let sps = sanitizeParameterSet(config.sps, codec: "h265", expectedNalType: 33)
        let pps = sanitizeParameterSet(config.pps, codec: "h265", expectedNalType: 34)
        guard !vps.isEmpty, !sps.isEmpty, !pps.isEmpty else { return nil }
        var format: CMFormatDescription?
        vps.withUnsafeBytes { vpsPtr in
            sps.withUnsafeBytes { spsPtr in
                pps.withUnsafeBytes { ppsPtr in
                    guard let vpsBase = vpsPtr.bindMemory(to: UInt8.self).baseAddress,
                          let spsBase = spsPtr.bindMemory(to: UInt8.self).baseAddress,
                          let ppsBase = ppsPtr.bindMemory(to: UInt8.self).baseAddress else {
                        return
                    }
                    var parameterSetPointers: [UnsafePointer<UInt8>] = [vpsBase, spsBase, ppsBase]
                    var parameterSetSizes = [vps.count, sps.count, pps.count]
                    let status = CMVideoFormatDescriptionCreateFromHEVCParameterSets(
                        allocator: kCFAllocatorDefault,
                        parameterSetCount: 3,
                        parameterSetPointers: &parameterSetPointers,
                        parameterSetSizes: &parameterSetSizes,
                        nalUnitHeaderLength: Int32(nalLength),
                        extensions: nil,
                        formatDescriptionOut: &format
                    )
                    if status != noErr {
                        logFormatFailure(
                            "buildHevcFormat",
                            status: status,
                            sets: [("vps", vps), ("sps", sps), ("pps", pps)]
                        )
                    }
                }
            }
        }
        return format
    }

    private func buildAudioFormat(config: AudioConfig) {
        let sampleRate = config.sampleRate ?? 44100
        let channels = config.channels ?? 2
        var format: CMAudioFormatDescription?
        let status: OSStatus

        if config.codec == "pcm_s16le" {
            let bytesPerFrame = UInt32(channels * 2)
            var asbd = AudioStreamBasicDescription(
                mSampleRate: Float64(sampleRate),
                mFormatID: kAudioFormatLinearPCM,
                mFormatFlags: kLinearPCMFormatFlagIsSignedInteger | kAudioFormatFlagIsPacked,
                mBytesPerPacket: bytesPerFrame,
                mFramesPerPacket: 1,
                mBytesPerFrame: bytesPerFrame,
                mChannelsPerFrame: UInt32(channels),
                mBitsPerChannel: 16,
                mReserved: 0
            )
            status = CMAudioFormatDescriptionCreate(
                allocator: kCFAllocatorDefault,
                asbd: &asbd,
                layoutSize: 0,
                layout: nil,
                magicCookieSize: 0,
                magicCookie: nil,
                extensions: nil,
                formatDescriptionOut: &format
            )
        } else {
            var asbd = AudioStreamBasicDescription(
                mSampleRate: Float64(sampleRate),
                mFormatID: kAudioFormatMPEG4AAC,
                mFormatFlags: 0,
                mBytesPerPacket: 0,
                mFramesPerPacket: 1024,
                mBytesPerFrame: 0,
                mChannelsPerFrame: UInt32(channels),
                mBitsPerChannel: 0,
                mReserved: 0
            )
            status = config.audioSpecificConfig.withUnsafeBytes { cookiePtr in
                CMAudioFormatDescriptionCreate(
                    allocator: kCFAllocatorDefault,
                    asbd: &asbd,
                    layoutSize: 0,
                    layout: nil,
                    magicCookieSize: cookiePtr.count,
                    magicCookie: cookiePtr.baseAddress,
                    extensions: nil,
                    formatDescriptionOut: &format
                )
            }
        }

        if status != noErr {
            emitError("audio format build failed: \(status)")
            return
        }
        audioFormatDescription = format
    }

    private func normalizeVideoTimes(dtsMs: UInt32, ptsMs: UInt32) -> (UInt32, UInt32) {
        if videoBaseTimeMs == nil {
            videoBaseTimeMs = min(dtsMs, ptsMs)
        }
        let base = videoBaseTimeMs ?? min(dtsMs, ptsMs)
        var normalizedDts = dtsMs >= base ? dtsMs - base : 0
        var normalizedPts = ptsMs >= base ? ptsMs - base : normalizedDts
        if let lastPts = lastVideoPtsMs, normalizedPts <= lastPts {
            let (next, overflow) = lastPts.addingReportingOverflow(1)
            normalizedPts = overflow ? lastPts : next
        }
        if let lastDts = lastVideoDtsMs, normalizedDts < lastDts {
            normalizedDts = lastDts
        }
        if normalizedDts > normalizedPts {
            normalizedDts = normalizedPts
        }
        lastVideoPtsMs = normalizedPts
        lastVideoDtsMs = normalizedDts
        return (normalizedDts, normalizedPts)
    }

    private func normalizeAudioTime(dtsMs: UInt32) -> UInt32 {
        if audioBaseTimeMs == nil {
            audioBaseTimeMs = dtsMs
        }
        let base = audioBaseTimeMs ?? dtsMs
        var normalized = dtsMs >= base ? dtsMs - base : 0
        if let lastPts = lastAudioPtsMs, normalized < lastPts {
            normalized = lastPts
        }
        lastAudioPtsMs = normalized
        return normalized
    }

    private func inferKeyframe(data: Data) -> Bool {
        guard let config = videoConfig else { return false }
        let codec = config.codec.lowercased()
        let format = config.format.lowercased()
        let nalLengthSize = config.nalLengthSize ?? 4

        if format == "avcc" {
            return inferKeyframeAvcc(data: data, codec: codec, nalLengthSize: nalLengthSize)
        }
        return inferKeyframeAnnexB(data: data, codec: codec)
    }

    private func inferKeyframeAvcc(data: Data, codec: String, nalLengthSize: Int) -> Bool {
        let lengthSize = max(1, min(4, nalLengthSize))
        var offset = 0
        while offset + lengthSize <= data.count {
            var nalLen: Int = 0
            for i in 0..<lengthSize {
                nalLen = (nalLen << 8) | Int(data[offset + i])
            }
            offset += lengthSize
            if nalLen <= 0 || offset + nalLen > data.count {
                break
            }
            if let header = data[offset..<(offset + 1)].first {
                if isKeyframeNalHeader(codec: codec, header: header) {
                    return true
                }
            }
            offset += nalLen
        }
        return false
    }

    private func inferKeyframeAnnexB(data: Data, codec: String) -> Bool {
        let bytes = [UInt8](data)
        var i = 0
        while i + 4 < bytes.count {
            var startCodeLen = 0
            if bytes[i] == 0 && bytes[i + 1] == 0 && bytes[i + 2] == 1 {
                startCodeLen = 3
            } else if bytes[i] == 0 && bytes[i + 1] == 0 && bytes[i + 2] == 0 && bytes[i + 3] == 1 {
                startCodeLen = 4
            }
            if startCodeLen == 0 {
                i += 1
                continue
            }
            let nalStart = i + startCodeLen
            if nalStart >= bytes.count {
                break
            }
            let header = bytes[nalStart]
            if isKeyframeNalHeader(codec: codec, header: header) {
                return true
            }
            i = nalStart + 1
        }
        return false
    }

    private func isKeyframeNalHeader(codec: String, header: UInt8) -> Bool {
        if codec == "h265" || codec == "hevc" {
            let nalType = (header >> 1) & 0x3F
            return nalType >= 16 && nalType <= 21
        }
        return (header & 0x1F) == 5
    }

    private func makeAudioSampleBuffer(
        data: Data,
        formatDescription: CMFormatDescription,
        dtsMs: UInt32,
        ptsMs: UInt32
    ) -> CMSampleBuffer? {
        if audioConfig?.codec == "pcm_s16le" {
            let sampleRate = audioConfig?.sampleRate ?? 44100
            let channels = audioConfig?.channels ?? 2
            let bytesPerFrame = max(1, channels * 2)
            let sampleCount = max(1, data.count / bytesPerFrame)
            let trimmedLength = sampleCount * bytesPerFrame
            let payload = trimmedLength == data.count ? data : data.prefix(trimmedLength)

            var blockBuffer: CMBlockBuffer?
            let status = CMBlockBufferCreateWithMemoryBlock(
                allocator: kCFAllocatorDefault,
                memoryBlock: nil,
                blockLength: payload.count,
                blockAllocator: kCFAllocatorDefault,
                customBlockSource: nil,
                offsetToData: 0,
                dataLength: payload.count,
                flags: 0,
                blockBufferOut: &blockBuffer
            )
            if status != kCMBlockBufferNoErr || blockBuffer == nil {
                return nil
            }
            payload.withUnsafeBytes { ptr in
                if let base = ptr.baseAddress {
                    CMBlockBufferReplaceDataBytes(
                        with: base,
                        blockBuffer: blockBuffer!,
                        offsetIntoDestination: 0,
                        dataLength: payload.count
                    )
                }
            }

            let pts = CMTime(value: CMTimeValue(ptsMs), timescale: 1000)
            let dts = CMTime(value: CMTimeValue(dtsMs), timescale: 1000)
            let duration = CMTime(value: 1, timescale: CMTimeScale(sampleRate))
            var timing = CMSampleTimingInfo(duration: duration, presentationTimeStamp: pts, decodeTimeStamp: dts)
            var sampleSize = bytesPerFrame
            var sampleBuffer: CMSampleBuffer?
            let sampleStatus = CMSampleBufferCreateReady(
                allocator: kCFAllocatorDefault,
                dataBuffer: blockBuffer,
                formatDescription: formatDescription,
                sampleCount: sampleCount,
                sampleTimingEntryCount: 1,
                sampleTimingArray: &timing,
                sampleSizeEntryCount: 1,
                sampleSizeArray: &sampleSize,
                sampleBufferOut: &sampleBuffer
            )
            if sampleStatus != noErr {
                return nil
            }
            return sampleBuffer
        }

        return makeSampleBuffer(
            data: data,
            formatDescription: formatDescription,
            dtsMs: dtsMs,
            ptsMs: ptsMs
        )
    }

    @MainActor
    private func suppressNativePlayback(in view: UIView) {
        suppressedLayers = suppressedLayers.filter { $0.value != nil }
        suppressedViews = suppressedViews.filter { $0.value != nil }

        let trackedLayers = Set(suppressedLayers.compactMap { $0.value }.map { ObjectIdentifier($0) })
        let trackedViews = Set(suppressedViews.compactMap { $0.value }.map { ObjectIdentifier($0) })
        if let layers = view.layer.sublayers {
            for layer in layers where layer is AVPlayerLayer {
                layer.isHidden = true
                if !trackedLayers.contains(ObjectIdentifier(layer)) {
                    suppressedLayers.append(WeakRef(layer))
                }
            }
        }
        for subview in view.subviews where subview is UIImageView {
            subview.isHidden = true
            if !trackedViews.contains(ObjectIdentifier(subview)) {
                suppressedViews.append(WeakRef(subview))
            }
        }
    }

    @MainActor
    private func restoreNativePlayback() {
        for layer in suppressedLayers.compactMap({ $0.value }) {
            layer.isHidden = false
        }
        for view in suppressedViews.compactMap({ $0.value }) {
            view.isHidden = false
        }
        suppressedLayers.removeAll()
        suppressedViews.removeAll()
    }

    @MainActor
    private func handleAppWillResignActive() {
        wasPlayingBeforeBackground = isPlaying
        renderSynchronizer.rate = 0.0
    }

    @MainActor
    private func handleAppDidBecomeActive() {
        if let view = containerView {
            suppressNativePlayback(in: view)
        }
        Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: 100_000_000)
            self?.ensureNativePlaybackSuppressed()
        }
        do {
            let session = AVAudioSession.sharedInstance()
            try session.setActive(true)
        } catch {
            os_log("Audio session re-activate failed: %{public}@", log: log, type: .error, String(describing: error))
        }
        if wasPlayingBeforeBackground {
            renderSynchronizer.rate = 1.0
            wasPlayingBeforeBackground = false
        }
    }

    @MainActor
    private func ensureNativePlaybackSuppressed() {
        guard let view = containerView else { return }
        suppressNativePlayback(in: view)
    }

    @MainActor
    private func ensureAudioSession(sampleRate: Int?) {
        if audioSessionConfigured {
            return
        }
        let session = AVAudioSession.sharedInstance()
        do {
            if let rate = sampleRate {
                try? session.setPreferredSampleRate(Double(rate))
            }
            try session.setCategory(.playback, mode: .default, options: [])
            try session.setActive(true)
            audioSessionConfigured = true
        } catch {
            os_log("Audio session setup failed: %{public}@", log: log, type: .error, String(describing: error))
        }
    }

    @MainActor
    private func setupPcmAudioEngine(sampleRate: Int, channels: Int) {
        guard audioEngine == nil else {
            usePcmAudioEngine = true
            applyStreamVolume()
            return
        }
        let format = AVAudioFormat(
            commonFormat: .pcmFormatInt16,
            sampleRate: Double(sampleRate),
            channels: AVAudioChannelCount(channels),
            interleaved: true
        )
        guard let format else {
            os_log("PCM audio engine format init failed", log: log, type: .error)
            return
        }
        let engine = AVAudioEngine()
        let player = AVAudioPlayerNode()
        engine.attach(player)
        engine.connect(player, to: engine.mainMixerNode, format: format)
        do {
            try engine.start()
            player.play()
            audioEngine = engine
            audioPlayer = player
            pcmAudioFormat = format
            usePcmAudioEngine = true
            applyStreamVolume()
        } catch {
            os_log("PCM audio engine start failed: %{public}@", log: log, type: .error, String(describing: error))
        }
    }

    private func stopPcmAudioEngine() {
        if let player = audioPlayer {
            player.stop()
        }
        audioEngine?.stop()
        audioEngine = nil
        audioPlayer = nil
        pcmAudioFormat = nil
        usePcmAudioEngine = false
    }

    private func enqueuePcmAudio(data: Data, format: AVAudioFormat) {
        guard let player = audioPlayer else { return }
        let channels = Int(format.channelCount)
        let bytesPerFrame = max(1, channels * 2)
        let sampleCount = max(1, data.count / bytesPerFrame)
        let frameCount = AVAudioFrameCount(sampleCount)
        guard let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frameCount) else {
            return
        }
        buffer.frameLength = frameCount
        let byteCount = sampleCount * bytesPerFrame
        data.withUnsafeBytes { src in
            let audioBufferList = buffer.audioBufferList.pointee
            if let dst = audioBufferList.mBuffers.mData, let base = src.baseAddress {
                memcpy(dst, base, byteCount)
            }
        }
        player.scheduleBuffer(buffer, at: nil, options: [], completionHandler: nil)
        if !player.isPlaying {
            player.play()
        }
    }

    @MainActor
    private func setStreamVolume(_ volume: Float) {
        streamVolume = max(0.0, min(1.0, volume))
        applyStreamVolume()
    }

    @MainActor
    private func applyStreamVolume() {
        let effective = streamMuted ? 0.0 : streamVolume
        audioPlayer?.volume = effective
        if let engine = audioEngine {
            engine.mainMixerNode.outputVolume = effective
        }
        audioRenderer.setValue(NSNumber(value: effective), forKey: "volume")
    }

    private func convertVideoData(_ data: Data) -> Data {
        guard let config = videoConfig else { return data }
        if config.format == "avcc" {
            return data
        }
        return annexBToAvcc(data: data, nalLengthSize: config.nalLengthSize ?? 4)
    }

    private func annexBToAvcc(data: Data, nalLengthSize: Int) -> Data {
        let bytes = [UInt8](data)
        var startIndices: [Int] = []
        var i = 0
        while i + 3 < bytes.count {
            if bytes[i] == 0 && bytes[i + 1] == 0 {
                if bytes[i + 2] == 1 {
                    startIndices.append(i)
                    i += 3
                    continue
                } else if bytes[i + 2] == 0 && bytes[i + 3] == 1 {
                    startIndices.append(i)
                    i += 4
                    continue
                }
            }
            i += 1
        }
        if startIndices.isEmpty {
            return data
        }

        var output = Data(capacity: data.count)
        for index in 0..<startIndices.count {
            let start = startIndices[index]
            let next = index + 1 < startIndices.count ? startIndices[index + 1] : bytes.count
            let prefixLen = bytes[start + 2] == 1 ? 3 : 4
            let nalStart = start + prefixLen
            let nalSize = max(0, next - nalStart)
            appendNalLength(nalSize, lengthSize: nalLengthSize, to: &output)
            output.append(contentsOf: bytes[nalStart..<nalStart + nalSize])
        }
        return output
    }

    private func appendNalLength(_ length: Int, lengthSize: Int, to data: inout Data) {
        var value = UInt32(length)
        let size = max(1, min(4, lengthSize))
        for i in (0..<size).reversed() {
            let shift = UInt32(i * 8)
            let byte = UInt8((value >> shift) & 0xFF)
            data.append(byte)
        }
    }

    private func makeSampleBuffer(
        data: Data,
        formatDescription: CMFormatDescription,
        dtsMs: UInt32,
        ptsMs: UInt32
    ) -> CMSampleBuffer? {
        var blockBuffer: CMBlockBuffer?
        let status = CMBlockBufferCreateWithMemoryBlock(
            allocator: kCFAllocatorDefault,
            memoryBlock: nil,
            blockLength: data.count,
            blockAllocator: kCFAllocatorDefault,
            customBlockSource: nil,
            offsetToData: 0,
            dataLength: data.count,
            flags: 0,
            blockBufferOut: &blockBuffer
        )
        if status != kCMBlockBufferNoErr || blockBuffer == nil {
            os_log(
                "CMBlockBufferCreateWithMemoryBlock failed status=%{public}@ len=%{public}@",
                log: log,
                type: .error,
                String(status),
                String(data.count)
            )
            return nil
        }
        data.withUnsafeBytes { ptr in
            if let base = ptr.baseAddress {
                CMBlockBufferReplaceDataBytes(
                    with: base,
                    blockBuffer: blockBuffer!,
                    offsetIntoDestination: 0,
                    dataLength: data.count
                )
            }
        }

        let pts = CMTime(value: CMTimeValue(ptsMs), timescale: 1000)
        let dts = CMTime(value: CMTimeValue(dtsMs), timescale: 1000)
        var timing = CMSampleTimingInfo(duration: .invalid, presentationTimeStamp: pts, decodeTimeStamp: dts)
        var sampleBuffer: CMSampleBuffer?
        var sampleSize = data.count
        let sampleStatus = CMSampleBufferCreateReady(
            allocator: kCFAllocatorDefault,
            dataBuffer: blockBuffer,
            formatDescription: formatDescription,
            sampleCount: 1,
            sampleTimingEntryCount: 1,
            sampleTimingArray: &timing,
            sampleSizeEntryCount: 1,
            sampleSizeArray: &sampleSize,
            sampleBufferOut: &sampleBuffer
        )
        if sampleStatus != noErr {
            os_log(
                "CMSampleBufferCreateReady failed status=%{public}@ len=%{public}@ pts=%{public}@ dts=%{public}@",
                log: log,
                type: .error,
                String(sampleStatus),
                String(data.count),
                String(ptsMs),
                String(dtsMs)
            )
            return nil
        }
        return sampleBuffer
    }

    private func emitError(_ message: String) {
        os_log("Stream decoder error %{public}@", log: log, type: .error, message)
        let componentId = self.componentId
        DispatchQueue.main.async {
            ComponentRouter.shared.emitComponentEvent(
                componentId: componentId,
                event: "error",
                detail: ["code": "stream_decoder", "message": message]
            )
        }
    }
}

private struct VideoConfig {
    let codec: String
    let format: String
    let sps: Data
    let pps: Data
    let vps: Data
    let nalLengthSize: Int?
    let width: Int?
    let height: Int?

    init?(json: String) {
        guard let data = json.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return nil
        }
        codec = (obj["codec"] as? String) ?? "h264"
        format = (obj["format"] as? String) ?? "annexb"
        sps = VideoConfig.parseBytes(obj["sps"])
        pps = VideoConfig.parseBytes(obj["pps"])
        vps = VideoConfig.parseBytes(obj["vps"])
        nalLengthSize = obj["nalLengthSize"] as? Int
        width = obj["width"] as? Int
        height = obj["height"] as? Int
    }

    static func parseBytes(_ value: Any?) -> Data {
        if let base64 = value as? String {
            if base64.isEmpty {
                return Data()
            }
            if let decoded = Data(base64Encoded: base64) {
                return decoded
            }
            return Data()
        }

        guard let array = value as? [Any] else { return Data() }
        var bytes: [UInt8] = []
        bytes.reserveCapacity(array.count)
        for item in array {
            if let num = item as? NSNumber {
                bytes.append(UInt8(truncating: num))
            }
        }
        return Data(bytes)
    }
}

private struct AudioConfig {
    let codec: String
    let audioSpecificConfig: Data
    let sampleRate: Int?
    let channels: Int?
    let aacIsAdts: Bool

    init?(json: String) {
        guard let data = json.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return nil
        }
        codec = (obj["codec"] as? String) ?? "aac"
        audioSpecificConfig = VideoConfig.parseBytes(obj["audioSpecificConfig"])
        sampleRate = obj["sampleRate"] as? Int
        channels = obj["channels"] as? Int
        aacIsAdts = obj["aacIsAdts"] as? Bool ?? false
    }
}

#endif
