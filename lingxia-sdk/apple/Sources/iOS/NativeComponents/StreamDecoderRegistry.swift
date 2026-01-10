import Foundation
import OSLog
import AVFoundation
import AudioToolbox
import CoreImage
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

        // When stream ends naturally, don't destroy the session - just pause and emit ended event.
        // Keep the last frame visible so user can see where playback stopped.
        // The session will be reused if user replays (seek to 0 + play).
        if let session = sessions[componentId] {
            os_log(
                "Stream decoder stop (keeping last frame) for %{public}@",
                log: log,
                type: .info,
                componentId
            )
            session.handleStreamEnded()
            return true
        }

        // Session not found - nothing to do
        let shouldForget = ComponentRouter.shared.componentView(componentId: componentId) == nil
        if shouldForget {
            lastVideoConfigJson.removeValue(forKey: componentId)
            lastAudioConfigJson.removeValue(forKey: componentId)
            stopping.remove(componentId)
        }
        return true
    }

    /// Fully destroy a decoder session (called when component is unmounted).
    func destroy(componentId: String) {
        stopping.insert(componentId)
        let session = sessions.removeValue(forKey: componentId)
        lastVideoConfigJson.removeValue(forKey: componentId)
        lastAudioConfigJson.removeValue(forKey: componentId)
        stopping.remove(componentId)
        os_log(
            "Stream decoder destroy for %{public}@",
            log: log,
            type: .info,
            componentId
        )
        session?.stop()
        ComponentRouter.shared.setStreamDecoderActive(componentId: componentId, active: false)
    }

    func handleCommand(componentId: String, name: String, params: [String: Any]?) -> Bool {
        stopping.remove(componentId)
        let session: StreamDecoderSession
        if let existing = sessions[componentId] {
            session = existing
        } else {
            guard shouldHandleCommand(componentId: componentId),
                  let created = ensureSession(componentId: componentId, reason: "handleCommand")
            else {
                return false
            }
            session = created
        }
        return session.handleCommand(name: name, params: params)
    }

    func hasSession(componentId: String) -> Bool {
        return sessions[componentId] != nil
    }

    /// Best-effort playback position for stream decoder mode (seconds).
    /// For live streams this is still a monotonic clock (relative to the most recent timeline base),
    /// so callers should only use it for UI/progress when a finite duration is known.
    func playbackPositionSeconds(componentId: String) -> Double? {
        sessions[componentId]?.playbackPositionSeconds()
    }

    func shouldHandleCommand(componentId: String) -> Bool {
        if sessions[componentId] != nil { return true }
        if lastVideoConfigJson[componentId] != nil { return true }
        if lastAudioConfigJson[componentId] != nil { return true }
        return false
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
    private var audioSessionSampleRate: Int?
    private var audioEngine: AVAudioEngine?
    private var audioPlayer: AVAudioPlayerNode?
    private var pcmAudioFormat: AVAudioFormat?
    private var usePcmAudioEngine = false
    private var aacConverter: AudioConverterRef?
    private var aacConverterEnabled = false
    private var streamVolume: Float = 1.0
    private var streamMuted = false
    private var waitingForVideoKeyframe = false
    private var resumeAfterVideoKeyframe = false
    private var playRequested = false
    private var hasEverDisplayedFrame = false // Track if any frame was ever displayed in this session
    private var gateAudioUntilVideo = false
    private var timelineResetPending = true
    private var suppressedLayers: [WeakRef<CALayer>] = []
    private var appObservers: [NSObjectProtocol] = []
    private var wasPlayingBeforeBackground = false
    private var pendingVideoKeyframe: (data: Data, dtsMs: UInt32, ptsMs: UInt32)?
    private var lastEnqueuedVideoPtsMs: UInt32?
    private var lastVideoFlushAt: CFAbsoluteTime = 0
    private var lastTimebaseCheckAt: CFAbsoluteTime = 0
    private var lastTimebaseMs: Int64?
    private var timebaseStallCount: Int = 0
    private var lastTimebaseResyncAt: CFAbsoluteTime = 0
    private var timebaseDriftCount: Int = 0
    private var videoEnqueueCount: UInt64 = 0
    private var lastDroppedKeyframeLogAt: CFAbsoluteTime = 0
    private var lastNonVclLogAt: CFAbsoluteTime = 0
    private var videoStuckTimer: DispatchSourceTimer?
    private var playBeganAt: CFAbsoluteTime?
    private var playNotifyScheduled = false
    private var lastDisplayedSignature: UInt64?
    private var lastDisplayedSignatureAt: CFAbsoluteTime = 0
    private var lastStuckCheckEnqueueCount: UInt64 = 0
    private var lastStuckRecoveryAt: CFAbsoluteTime = 0
    private var freezeOverlayView: UIImageView?
    private var freezeOverlayShownAt: CFAbsoluteTime = 0
    private var freezeOverlayEnqueueCount: UInt64 = 0
    private var freezeOverlaySignature: UInt64?

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
        renderSynchronizer.setRate(0.0, time: .zero)
        timelineResetPending = true

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
        appObservers.append(
            center.addObserver(
                forName: Notification.Name("AVSampleBufferDisplayLayerFailedToDecodeNotification"),
                object: videoLayerView.displayLayer,
                queue: .main
            ) { [weak self] notification in
                let infoDescription = String(describing: notification.userInfo ?? [:])
                Task { @MainActor in
                    self?.handleVideoDecodeFailure(infoDescription: infoDescription)
                }
            }
        )

        startVideoStuckMonitor()
    }

    func usesContainerView(_ view: UIView) -> Bool {
        containerView === view
    }

    /// Best-effort current playback position (seconds) based on the last normalized video PTS.
    /// This is intended for UI/progress only.
    @MainActor
    func playbackPositionSeconds() -> Double {
        let ptsMs: UInt32 = decodeQueue.sync { self.lastVideoPtsMs ?? 0 }
        let ptsSeconds = Double(ptsMs) / 1000.0

        // Prefer timebase for smooth interpolation, but never run ahead of the last known PTS
        // (otherwise the UI progress can reach 100% while video is still buffering/stalled).
        let tb = CMTimebaseGetTime(renderSynchronizer.timebase)
        if tb.isValid, tb.isNumeric {
            return min(max(0, tb.seconds), ptsSeconds)
        }
        return ptsSeconds
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
        if let existing = audioConfig, existing == config {
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
            aacConverterEnabled = false
            if let converter = aacConverter {
                AudioConverterDispose(converter)
                aacConverter = nil
            }
        } else if config.codec == "aac" {
            // iOS has been unreliable with AVSampleBufferAudioRenderer for some AAC variants (noise
            // then silence). Prefer decoding AAC to PCM via AudioConverter and feed the existing
            // PCM audio engine path.
            let sampleRate = config.sampleRate ?? 44100
            let channels = config.channels ?? 1
            setupPcmAudioEngine(sampleRate: sampleRate, channels: channels)
            buildAacConverter(sampleRate: sampleRate, channels: channels, asc: config.audioSpecificConfig)
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
                if let config = self.videoConfig, !self.isValidVideoAccessUnit(data: data, config: config, requireKeyframe: true) {
                    let now = CFAbsoluteTimeGetCurrent()
                    if now - self.lastDroppedKeyframeLogAt > 1.0 {
                        self.lastDroppedKeyframeLogAt = now
                        os_log(
                            "dropping initial keyframe (invalid access unit) componentId=%{public}@ len=%{public}@ codec=%{public}@ fmt=%{public}@",
                            log: self.log,
                            type: .error,
                            self.componentId,
                            String(data.count),
                            config.codec,
                            config.format
                        )
                    }
                    // Some cameras can emit a truncated/garbled first keyframe right after connect.
                    // Dropping it and waiting for the next clean keyframe avoids "first frame only"
                    // playback that gets fixed by a manual pause/play.
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
            let durationMs: UInt32 = if let last = self.lastEnqueuedVideoPtsMs,
                                          normalizedPts > last
            {
                normalizedPts - last
            } else {
                33
            }
            self.lastEnqueuedVideoPtsMs = normalizedPts
            let sampleData = self.convertVideoData(data)

            if let config = self.videoConfig {
                // Some streams emit access units that contain only SPS/PPS/SEI (no VCL picture). Those
                // do not advance the rendered image and can confuse first-play scheduling on iOS.
                if !self.isValidVideoAccessUnit(data: sampleData, config: config, requireKeyframe: false) {
                    let now = CFAbsoluteTimeGetCurrent()
                    if now - self.lastNonVclLogAt > 1.0 {
                        self.lastNonVclLogAt = now
                        os_log(
                            "dropping non-VCL access unit componentId=%{public}@ len=%{public}@ codec=%{public}@ fmt=%{public}@",
                            log: self.log,
                            type: .info,
                            self.componentId,
                            String(sampleData.count),
                            config.codec,
                            config.format
                        )
                    }
                    return
                }
            }
            guard let sampleBuffer = self.makeSampleBuffer(
                data: sampleData,
                formatDescription: format,
                dtsMs: normalizedDts,
                ptsMs: normalizedPts,
                durationMs: durationMs
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
            self.applyVideoSampleAttachments(sampleBuffer, isSync: keyframe || self.inferKeyframe(data: sampleData))
            self.videoLayerView.displayLayer.enqueue(sampleBuffer)
            self.maybeResyncTimebase(to: sampleBuffer.presentationTimeStamp)

            self.videoEnqueueCount &+= 1
            if #available(iOS 11.0, *) {
                if self.videoLayerView.displayLayer.requiresFlushToResumeDecoding {
                    let now = CFAbsoluteTimeGetCurrent()
                    if now - self.lastVideoFlushAt > 0.5 {
                        self.lastVideoFlushAt = now
                        os_log(
                            "video layer requires flush componentId=%{public}@",
                            log: self.log,
                            type: .error,
                            self.componentId
                        )
                    }
                    self.lastEnqueuedVideoPtsMs = nil
                    self.waitingForVideoKeyframe = true
                    self.resumeAfterVideoKeyframe = self.playRequested
                    self.timelineResetPending = true
                    DispatchQueue.main.async {
                        self.videoLayerView.displayLayer.flush()
                        ComponentRouter.shared.emitComponentEvent(
                            componentId: self.componentId,
                            event: "waiting",
                            detail: ["reason": "decode"]
                        )
                    }
                    return
                }
            }
            if self.videoLayerView.displayLayer.status == .failed {
                self.emitError("video layer failed: \(String(describing: self.videoLayerView.displayLayer.error))")
                return
            }
            self.maybeNotifyPlay()
            if self.playRequested && self.renderSynchronizer.rate == 0.0 && !self.waitingForVideoKeyframe {
                if self.timelineResetPending {
                    self.timelineResetPending = false
                    self.renderSynchronizer.setRate(1.0, time: sampleBuffer.presentationTimeStamp)
                } else {
                    self.renderSynchronizer.rate = 1.0
                }
            }
        }
    }

    private func applyVideoSampleAttachments(_ sampleBuffer: CMSampleBuffer, isSync: Bool) {
        guard let attachments = CMSampleBufferGetSampleAttachmentsArray(sampleBuffer, createIfNecessary: true) else {
            return
        }
        guard CFArrayGetCount(attachments) > 0 else { return }
        let dict = unsafeBitCast(CFArrayGetValueAtIndex(attachments, 0), to: CFMutableDictionary.self)
        CFDictionarySetValue(
            dict,
            Unmanaged.passUnretained(kCMSampleAttachmentKey_NotSync).toOpaque(),
            Unmanaged.passUnretained(isSync ? kCFBooleanFalse : kCFBooleanTrue).toOpaque()
        )
    }

    private func maybeResyncTimebase(to pts: CMTime) {
        guard playRequested, renderSynchronizer.rate != 0.0, !waitingForVideoKeyframe else { return }
        let now = CFAbsoluteTimeGetCurrent()
        if now - lastTimebaseCheckAt < 0.5 {
            return
        }
        lastTimebaseCheckAt = now

        let timebaseNow = CMTimebaseGetTime(renderSynchronizer.timebase)
        guard timebaseNow.isNumeric else { return }
        let scaled = CMTimeConvertScale(timebaseNow, timescale: 1000, method: .default)
        let timebaseMs = scaled.value
        let ptsMs = CMTimeConvertScale(pts, timescale: 1000, method: .default).value

        if let last = lastTimebaseMs {
            if timebaseMs <= last {
                timebaseStallCount += 1
            } else {
                timebaseStallCount = 0
            }
        }
        lastTimebaseMs = timebaseMs

        if ptsMs > timebaseMs + 500 {
            timebaseDriftCount += 1
        } else {
            timebaseDriftCount = 0
        }

        if timebaseStallCount >= 3 && now - lastTimebaseResyncAt > 1.0 {
            timebaseStallCount = 0
            timebaseDriftCount = 0
            lastTimebaseResyncAt = now
            os_log(
                "render timebase stalled; resyncing componentId=%{public}@ time=%{public}@ pts=%{public}@",
                log: log,
                type: .error,
                componentId,
                String(timebaseMs),
                String(ptsMs)
            )
            timelineResetPending = false
            renderSynchronizer.setRate(1.0, time: pts)
        } else if timebaseDriftCount >= 3 && now - lastTimebaseResyncAt > 1.0 {
            timebaseStallCount = 0
            timebaseDriftCount = 0
            lastTimebaseResyncAt = now
            os_log(
                "render timebase drift; resyncing componentId=%{public}@ time=%{public}@ pts=%{public}@",
                log: log,
                type: .error,
                componentId,
                String(timebaseMs),
                String(ptsMs)
            )
            timelineResetPending = false
            renderSynchronizer.setRate(1.0, time: pts)
        }
    }

    private func maybeNotifyPlay() {
        guard playRequested, !isPlaying else { return }

        let status = videoLayerView.displayLayer.status
        let now = CFAbsoluteTimeGetCurrent()
        let fallback = playBeganAt.map { now - $0 > 0.25 } ?? false
        guard status == .rendering || fallback else {
            schedulePlayNotifyCheck()
            return
        }

        isPlaying = true
        hasEverDisplayedFrame = true
        gateAudioUntilVideo = false
        let componentId = componentId
        DispatchQueue.main.async {
            ComponentRouter.shared.emitComponentEvent(
                componentId: componentId,
                event: "playing",
                detail: [:]
            )
        }
    }

    private func schedulePlayNotifyCheck() {
        guard !playNotifyScheduled else { return }
        playNotifyScheduled = true
        decodeQueue.asyncAfter(deadline: .now() + 0.05) { [weak self] in
            guard let self else { return }
            self.playNotifyScheduled = false
            self.maybeNotifyPlay()
        }
    }

    private func isValidVideoAccessUnit(data: Data, config: VideoConfig, requireKeyframe: Bool) -> Bool {
        let codec = config.codec.lowercased()
        let format = config.format.lowercased()
        let nalLengthSize = max(1, min(4, config.nalLengthSize ?? 4))

        var hasVcl = false
        var hasKeyframe = false
        var totalVclBytes = 0

        if format == "avcc" {
            var offset = 0
            while offset + nalLengthSize <= data.count {
                var nalLen = 0
                for i in 0..<nalLengthSize {
                    nalLen = (nalLen << 8) | Int(data[offset + i])
                }
                offset += nalLengthSize
                if nalLen <= 0 || offset + nalLen > data.count {
                    return false
                }
                let header = data[offset]
                if !isValidNalHeader(codec: codec, header: header) {
                    return false
                }
                if codec == "h265" || codec == "hevc" {
                    let nalType = Int((header >> 1) & 0x3F)
                    if nalType <= 31 {
                        hasVcl = true
                        totalVclBytes += nalLen
                    }
                } else {
                    let nalType = Int(header & 0x1F)
                    if (1...5).contains(nalType) {
                        hasVcl = true
                        totalVclBytes += nalLen
                    }
                }
                if isKeyframeNalHeader(codec: codec, header: header) {
                    hasKeyframe = true
                }
                offset += nalLen
            }
            if offset != data.count {
                // Trailing bytes that don't form a full NAL length prefix.
                return false
            }
        } else {
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
                if !isValidNalHeader(codec: codec, header: header) {
                    return false
                }
                // Find next start code.
                var j = nalStart + 1
                while j + 3 < bytes.count {
                    if bytes[j] == 0 && bytes[j + 1] == 0 && (bytes[j + 2] == 1 || (bytes[j + 2] == 0 && bytes[j + 3] == 1)) {
                        break
                    }
                    j += 1
                }
                let nalLen = max(1, j - nalStart)
                if codec == "h265" || codec == "hevc" {
                    let nalType = Int((header >> 1) & 0x3F)
                    if nalType <= 31 {
                        hasVcl = true
                        totalVclBytes += nalLen
                    }
                } else {
                    let nalType = Int(header & 0x1F)
                    if (1...5).contains(nalType) {
                        hasVcl = true
                        totalVclBytes += nalLen
                    }
                }
                if isKeyframeNalHeader(codec: codec, header: header) {
                    hasKeyframe = true
                }
                i = j
            }
        }

        if requireKeyframe && !hasKeyframe {
            return false
        }
        if !hasVcl {
            return false
        }
        if requireKeyframe && totalVclBytes < 64 {
            // Avoid accepting tiny/truncated "keyframes" right after connect.
            return false
        }
        return true
    }

    func pushAudio(data: Data, dtsMs: UInt32, ptsMs: UInt32) {
        decodeQueue.async { [weak self] in
            guard let self = self else { return }
            if self.gateAudioUntilVideo || self.waitingForVideoKeyframe {
                return
            }

            if self.aacConverterEnabled, let audioFormat = self.pcmAudioFormat {
                guard let pcm = self.decodeAacToPcm(data) else {
                    return
                }
                DispatchQueue.main.async {
                    self.enqueuePcmAudio(data: pcm, format: audioFormat)
                }
                return
            }

            guard let format = self.audioFormatDescription else { return }
            if self.usePcmAudioEngine, let audioFormat = self.pcmAudioFormat {
                DispatchQueue.main.async {
                    self.enqueuePcmAudio(data: data, format: audioFormat)
                }
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
                let (payload, samplesPerPacket) = self.extractAacPayload(data)
                sampleBuffer = self.makeAudioSampleBuffer(
                    data: payload,
                    formatDescription: format,
                    dtsMs: normalizedDts,
                    ptsMs: normalizedDts,
                    aacSamplesPerPacket: samplesPerPacket
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
            if self.audioRenderer.status == .failed {
                self.emitError("audio renderer failed: \(String(describing: self.audioRenderer.error))")
                return
            }
            if self.renderSynchronizer.rate == 0.0
                && self.isPlaying
                && self.playRequested
                && !self.waitingForVideoKeyframe
                && !self.gateAudioUntilVideo
            {
                if self.timelineResetPending {
                    self.timelineResetPending = false
                    self.renderSynchronizer.setRate(1.0, time: sampleBuffer.presentationTimeStamp)
                } else {
                    self.renderSynchronizer.rate = 1.0
                }
            }
        }
    }

    private func buildAacConverter(sampleRate: Int, channels: Int, asc: Data) {
        decodeQueue.async { [weak self] in
            guard let self else { return }

            if let existing = self.aacConverter {
                AudioConverterDispose(existing)
                self.aacConverter = nil
            }

            func aacObjectType(from asc: Data) -> Int? {
                if asc.isEmpty {
                    return nil
                }
                let bytes = [UInt8](asc)
                var bitOffset = 0
                func readBits(_ count: Int) -> Int? {
                    guard count > 0 else { return 0 }
                    guard bitOffset + count <= bytes.count * 8 else { return nil }
                    var value = 0
                    for _ in 0..<count {
                        let byteIndex = bitOffset / 8
                        let bitIndex = 7 - (bitOffset % 8)
                        let bit = (bytes[byteIndex] >> bitIndex) & 0x01
                        value = (value << 1) | Int(bit)
                        bitOffset += 1
                    }
                    return value
                }

                guard var objectType = readBits(5) else { return nil }
                if objectType == 31 {
                    guard let ext = readBits(6) else { return nil }
                    objectType = 32 + ext
                }
                return objectType
            }

            let objectType = aacObjectType(from: asc) ?? 2
            if objectType == 5 || objectType == 29 {
                os_log(
                    "AAC AudioSpecificConfig objectType=%{public}@ (SBR/HE-AAC)",
                    log: self.log,
                    type: .info,
                    String(objectType)
                )
            } else {
                os_log(
                    "AAC AudioSpecificConfig objectType=%{public}@",
                    log: self.log,
                    type: .info,
                    String(objectType)
                )
            }
            var input = AudioStreamBasicDescription(
                mSampleRate: Float64(sampleRate),
                mFormatID: kAudioFormatMPEG4AAC,
                mFormatFlags: UInt32(max(0, objectType)),
                mBytesPerPacket: 0,
                mFramesPerPacket: 1024,
                mBytesPerFrame: 0,
                mChannelsPerFrame: UInt32(max(1, channels)),
                mBitsPerChannel: 0,
                mReserved: 0
            )

            let bytesPerFrame = UInt32(max(1, channels) * 2)
            var output = AudioStreamBasicDescription(
                mSampleRate: Float64(sampleRate),
                mFormatID: kAudioFormatLinearPCM,
                mFormatFlags: kLinearPCMFormatFlagIsSignedInteger | kAudioFormatFlagIsPacked,
                mBytesPerPacket: bytesPerFrame,
                mFramesPerPacket: 1,
                mBytesPerFrame: bytesPerFrame,
                mChannelsPerFrame: UInt32(max(1, channels)),
                mBitsPerChannel: 16,
                mReserved: 0
            )

            var converter: AudioConverterRef?
            let status = AudioConverterNew(&input, &output, &converter)
            guard status == noErr, let converter else {
                self.aacConverterEnabled = false
                self.emitError("AAC converter init failed: \(status)")
                return
            }

            if !asc.isEmpty {
                asc.withUnsafeBytes { cookiePtr in
                    if let base = cookiePtr.baseAddress {
                        AudioConverterSetProperty(
                            converter,
                            kAudioConverterDecompressionMagicCookie,
                            UInt32(cookiePtr.count),
                            base
                        )
                    }
                }
            }

            self.aacConverter = converter
            self.aacConverterEnabled = true
        }
    }

    private final class AacConverterInput {
        var payload: Data
        var consumed = false
        let channels: UInt32
        let packetDescPtr: UnsafeMutablePointer<AudioStreamPacketDescription>

        init(payload: Data, channels: UInt32) {
            self.payload = payload
            self.channels = channels
            self.packetDescPtr = .allocate(capacity: 1)
            self.packetDescPtr.initialize(to: AudioStreamPacketDescription(
                mStartOffset: 0,
                mVariableFramesInPacket: 0,
                mDataByteSize: UInt32(payload.count)
            ))
        }

        deinit {
            packetDescPtr.deinitialize(count: 1)
            packetDescPtr.deallocate()
        }
    }

    private func decodeAacToPcm(_ data: Data) -> Data? {
        guard let converter = aacConverter, let config = audioConfig else {
            return nil
        }

        let (payload, samplesPerPacket) = extractAacPayload(data)
        if payload.isEmpty {
            return nil
        }

        let channels = UInt32(max(1, config.channels ?? 1))
        let bytesPerFrame = max(1, Int(channels) * 2)
        let frameCount = max(1, samplesPerPacket)
        var outBytes = [UInt8](repeating: 0, count: frameCount * bytesPerFrame)

        let input = AacConverterInput(payload: payload, channels: channels)
        let unmanaged = Unmanaged.passRetained(input)
        defer { unmanaged.release() }

        var ioOutputPackets: UInt32 = UInt32(frameCount)
        let status = outBytes.withUnsafeMutableBytes { outPtr -> OSStatus in
            guard let outBase = outPtr.baseAddress else { return -1 }
            var outAbl = AudioBufferList(
                mNumberBuffers: 1,
                mBuffers: AudioBuffer(
                    mNumberChannels: channels,
                    mDataByteSize: UInt32(outPtr.count),
                    mData: outBase
                )
            )

            return AudioConverterFillComplexBuffer(
                converter,
                { (
                    _,
                    ioNumPackets,
                    ioData,
                    outPacketDesc,
                    inUserData
                ) -> OSStatus in
                    guard let inUserData else {
                        return -1
                    }
                    let ctx = Unmanaged<AacConverterInput>.fromOpaque(inUserData).takeUnretainedValue()
                    if ctx.consumed {
                        ioNumPackets.pointee = 0
                        return noErr
                    }
                    ctx.consumed = true

                    return ctx.payload.withUnsafeBytes { inPtr in
                        guard let base = inPtr.baseAddress else { return -1 }
                        ctx.packetDescPtr.pointee.mDataByteSize = UInt32(inPtr.count)

                        ioNumPackets.pointee = 1
                        ioData.pointee.mNumberBuffers = 1
                        ioData.pointee.mBuffers.mNumberChannels = ctx.channels
                        ioData.pointee.mBuffers.mDataByteSize = UInt32(inPtr.count)
                        ioData.pointee.mBuffers.mData = UnsafeMutableRawPointer(mutating: base)

                        if let outPacketDesc {
                            outPacketDesc.pointee = ctx.packetDescPtr
                        }
                        return noErr
                    }
                },
                unmanaged.toOpaque(),
                &ioOutputPackets,
                &outAbl,
                nil
            )
        }

        if status != noErr {
            os_log("AAC converter decode failed: %{public}@", log: log, type: .error, String(status))
            return nil
        }

        let producedFrames = Int(ioOutputPackets)
        if producedFrames <= 0 {
            return nil
        }
        return Data(outBytes.prefix(producedFrames * bytesPerFrame))
    }

    private struct AdtsHeaderInfo {
        let headerLength: Int
        let frameLength: Int
        let samplesPerPacket: Int
    }

    private func parseAdtsHeader(_ data: Data) -> AdtsHeaderInfo? {
        if data.count < 7 {
            return nil
        }
        let b0 = data[data.startIndex]
        let b1 = data[data.startIndex + 1]
        if b0 != 0xFF || (b1 & 0xF0) != 0xF0 {
            return nil
        }

        let protectionAbsent = (b1 & 0x01) != 0
        let headerLen = protectionAbsent ? 7 : 9
        if data.count < headerLen {
            return nil
        }

        let b2 = data[data.startIndex + 2]
        let b3 = data[data.startIndex + 3]
        let b4 = data[data.startIndex + 4]
        let b5 = data[data.startIndex + 5]
        let b6 = data[data.startIndex + 6]

        let samplingIndex = (b2 >> 2) & 0x0F
        if samplingIndex > 12 {
            return nil
        }

        let channelConfig = ((b2 & 0x01) << 2) | ((b3 >> 6) & 0x03)
        if channelConfig > 7 {
            return nil
        }

        let frameLength = (Int(b3 & 0x03) << 11) | (Int(b4) << 3) | (Int((b5 >> 5) & 0x07))
        if frameLength < headerLen || frameLength > data.count {
            return nil
        }

        let rawBlocks = Int(b6 & 0x03) + 1
        let samples = max(1, rawBlocks * 1024)

        return AdtsHeaderInfo(headerLength: headerLen, frameLength: frameLength, samplesPerPacket: samples)
    }

    private func extractAacPayload(_ data: Data) -> (Data, Int) {
        let declaredAdts = audioConfig?.aacIsAdts ?? false
        if !declaredAdts {
            // iOS expects raw AAC access units (no ADTS); avoid header sniffing because raw AAC can
            // coincidentally match the ADTS syncword and corrupt audio (loud noise).
            return (data, 1024)
        }

        guard let header = parseAdtsHeader(data) else {
            return (data, 1024)
        }

        let frameEnd = min(data.count, max(header.headerLength, header.frameLength))
        if frameEnd <= header.headerLength {
            return (data, header.samplesPerPacket)
        }
        return (data.subdata(in: header.headerLength..<frameEnd), header.samplesPerPacket)
    }

    @MainActor
    func handleCommand(name: String, params: [String: Any]?) -> Bool {
        switch name {
        case "play":
            os_log("StreamDecoderSession handleCommand play componentId=%{public}@", log: log, type: .info, componentId)
            let hasDisplayedFrame = videoLayerView.displayLayer.status == .rendering
            playRequested = true
            playBeganAt = CFAbsoluteTimeGetCurrent()
            ensureAudioSession(sampleRate: audioConfig?.sampleRate)
            if let view = containerView {
                suppressNativePlayback(in: view)
            }
            updateCornerRadius()
            applyStreamVolume()
            if waitingForVideoKeyframe || !isPlaying {
                // When resuming after a pause, the stream provider may restart and the incoming
                // timestamps can reset. Reset our local timeline and wait for a fresh keyframe so
                // the display layer doesn't get stuck waiting on a far-future PTS.
                resetTimingForResume(keepLastFrame: hasDisplayedFrame)
                renderSynchronizer.rate = 0.0
                ComponentRouter.shared.emitComponentEvent(
                    componentId: componentId,
                    event: "waiting",
                    detail: ["reason": "buffering"]
                )
            } else {
                renderSynchronizer.rate = 1.0
            }
            return true
        case "pause":
            os_log("StreamDecoderSession handleCommand pause componentId=%{public}@", log: log, type: .info, componentId)
            playRequested = false
            renderSynchronizer.rate = 0.0
            isPlaying = false
            let emitEvent = (params?["emitEvent"] as? Bool) ?? true
            let reason = (params?["reason"] as? String) ?? "user"
            var detail: [String: Any] = ["reason": reason]
            if let current = params?["currentTime"] as? Double, current.isFinite, current >= 0 {
                detail["currentTime"] = current
            } else {
                detail["currentTime"] = playbackPositionSeconds()
            }
            if emitEvent {
                ComponentRouter.shared.emitComponentEvent(componentId: componentId, event: "pause", detail: detail)
            }
            return true
        case "stop":
            stopPlayback()
            ComponentRouter.shared.emitComponentEvent(componentId: componentId, event: "stop", detail: ["reason": "user"])
            return true
        case "streamEnded":
            // Stream ended naturally - pause decoder but keep last frame visible
            handleStreamEnded()
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
    private func resetTimingForResume(keepLastFrame: Bool) {
        resetVideoStuckState()
        gateAudioUntilVideo = true
        resumeAfterVideoKeyframe = true
        waitingForVideoKeyframe = true
        pendingVideoKeyframe = nil
        lastEnqueuedVideoPtsMs = nil

        videoBaseTimeMs = nil
        audioBaseTimeMs = nil
        lastVideoPtsMs = nil
        lastVideoDtsMs = nil
        lastAudioPtsMs = nil
        pcmAudioPtsMs = 0

        decodeQueue.async { [weak self] in
            guard let self = self else { return }
            if keepLastFrame {
                // Keep the last rendered frame visible while paused; just flush queued buffers so we
                // can resume cleanly on the next keyframe.
                self.videoLayerView.displayLayer.flush()
            } else {
                // Initial start / hard restart: fully reset the layer to avoid getting stuck after
                // previous failures.
                os_log(
                    "resetTimingForResume flushAndRemoveImage componentId=%{public}@",
                    log: self.log,
                    type: .info,
                    self.componentId
                )
                self.videoLayerView.displayLayer.flushAndRemoveImage()
            }
            self.audioRenderer.flush()
        }
        renderSynchronizer.setRate(0.0, time: .zero)
        timelineResetPending = true
    }

    /// Handle natural stream end - pause decoder but keep last frame visible.
    /// Emits "ended" event to notify UI layer.
    @MainActor
    func handleStreamEnded() {
        os_log("StreamDecoderSession handleStreamEnded componentId=%{public}@", log: log, type: .info, componentId)

        // Pause playback but keep the last frame visible
        playRequested = false
        renderSynchronizer.rate = 0.0
        isPlaying = false
        // hasEverDisplayedFrame stays true so resetStream knows to keep the frame

        // Flush pending buffers but keep the displayed frame
        decodeQueue.async { [weak self] in
            self?.videoLayerView.displayLayer.flush()
            self?.audioRenderer.flush()
        }

        // Emit ended event to UI
        ComponentRouter.shared.emitComponentEvent(
            componentId: componentId,
            event: "ended",
            detail: ["reason": "stream"]
        )
    }

    @MainActor
    private func stopPlayback(keepLastFrame: Bool = false) {
        resetVideoStuckState()
        renderSynchronizer.setRate(0.0, time: .zero)
        isPlaying = false
        waitingForVideoKeyframe = false
        resumeAfterVideoKeyframe = false
        playRequested = false
        if !keepLastFrame {
            hasEverDisplayedFrame = false // Only reset if we're clearing the display
        }
        gateAudioUntilVideo = false
        pendingVideoKeyframe = nil
        lastEnqueuedVideoPtsMs = nil

        videoBaseTimeMs = nil
        audioBaseTimeMs = nil
        lastVideoPtsMs = nil
        lastVideoDtsMs = nil
        lastAudioPtsMs = nil
        pcmAudioPtsMs = 0

        decodeQueue.async { [weak self] in
            if keepLastFrame {
                // Keep the last frame visible - just flush pending buffers
                self?.videoLayerView.displayLayer.flush()
            } else {
                self?.videoLayerView.displayLayer.flushAndRemoveImage()
            }
            self?.audioRenderer.flush()
        }
        timelineResetPending = true

        if let converter = aacConverter {
            AudioConverterDispose(converter)
            aacConverter = nil
            aacConverterEnabled = false
        }
        stopPcmAudioEngine()
        restoreNativePlayback()
    }

    @MainActor
    private func resetStream(hard: Bool) {
        resetVideoStuckState()
        // Use hasEverDisplayedFrame to reliably track if we've shown content (displayLayer.status may change after pause)
        let wasRunning = playRequested || renderSynchronizer.rate != 0.0 || isPlaying || hasEverDisplayedFrame
        renderSynchronizer.setRate(0.0, time: .zero)
        isPlaying = false

        if wasRunning { playRequested = true }

        waitingForVideoKeyframe = true
        resumeAfterVideoKeyframe = wasRunning
        gateAudioUntilVideo = wasRunning
        pendingVideoKeyframe = nil
        lastEnqueuedVideoPtsMs = nil

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
            if wasRunning {
                // Reset during/after playback: keep the last frame visible to avoid a black flash.
                self.videoLayerView.displayLayer.flush()
            } else {
                os_log(
                    "resetStream flushAndRemoveImage componentId=%{public}@ hard=%{public}@",
                    log: self.log,
                    type: .info,
                    self.componentId,
                    String(hard)
                )
                self.videoLayerView.displayLayer.flushAndRemoveImage()
            }
            self.audioRenderer.flush()
        }
        timelineResetPending = true

        if let view = containerView {
            suppressNativePlayback(in: view)
        }
    }

    @MainActor
    private func startVideoStuckMonitor() {
        if videoStuckTimer != nil { return }
        let timer = DispatchSource.makeTimerSource(queue: .main)
        timer.schedule(deadline: .now() + 1.0, repeating: 0.5)
        timer.setEventHandler { [weak self] in
            self?.checkVideoStuck()
        }
        videoStuckTimer = timer
        timer.resume()
    }

    @MainActor
    private func resetVideoStuckState() {
        playBeganAt = nil
        lastDisplayedSignature = nil
        lastDisplayedSignatureAt = 0
        lastStuckCheckEnqueueCount = 0
        hideFreezeOverlay()
    }

    @MainActor
    private func checkVideoStuck() {
        guard let beganAt = playBeganAt else { return }
        let now = CFAbsoluteTimeGetCurrent()
        if now - beganAt > 10.0 { return }
        if now - lastStuckRecoveryAt < 2.0 { return }

        if videoLayerView.displayLayer.status != .rendering {
            return
        }

        let snapshot: (playRequested: Bool, waitingForKeyframe: Bool, enqueueCount: UInt64) = decodeQueue.sync {
            (self.playRequested, self.waitingForVideoKeyframe, self.videoEnqueueCount)
        }
        maybeHideFreezeOverlay(now: now, enqueueCount: snapshot.enqueueCount, waitingForKeyframe: snapshot.waitingForKeyframe)
        guard snapshot.playRequested, !snapshot.waitingForKeyframe else { return }
        guard snapshot.enqueueCount >= 12 else { return }

        guard let pixelBuffer = copyDisplayedPixelBufferIfAvailable() else { return }
        let signature = pixelBufferSignature(pixelBuffer)

        if let last = lastDisplayedSignature {
            if signature != last {
                lastDisplayedSignature = signature
                lastDisplayedSignatureAt = now
                lastStuckCheckEnqueueCount = snapshot.enqueueCount
                return
            }

            if lastDisplayedSignatureAt == 0 {
                lastDisplayedSignatureAt = now
                lastStuckCheckEnqueueCount = snapshot.enqueueCount
                return
            }

            let enqueuedDelta = snapshot.enqueueCount > lastStuckCheckEnqueueCount
                ? snapshot.enqueueCount - lastStuckCheckEnqueueCount
                : 0
            if enqueuedDelta >= 12 && now - lastDisplayedSignatureAt > 1.5 {
                lastStuckRecoveryAt = now
                os_log(
                    "video appears stuck; auto-resetting timeline componentId=%{public}@ enqueue=%{public}@",
                    log: log,
                    type: .error,
                    componentId,
                    String(snapshot.enqueueCount)
                )
                showFreezeOverlay(reason: "stuck")
                // Keep the last rendered frame visible during recovery to avoid a black flash.
                resetTimingForResume(keepLastFrame: true)
                ComponentRouter.shared.emitComponentEvent(
                    componentId: componentId,
                    event: "waiting",
                    detail: ["reason": "stuck"]
                )
            }
            return
        }

        lastDisplayedSignature = signature
        lastDisplayedSignatureAt = now
        lastStuckCheckEnqueueCount = snapshot.enqueueCount
    }

    @MainActor
    private func showFreezeOverlay(reason: StaticString) {
        guard freezeOverlayView == nil else { return }
        guard let containerView else { return }
        guard videoLayerView.bounds.width > 1, videoLayerView.bounds.height > 1 else { return }

        // Try to snapshot the currently rendered frame so decoder recovery doesn't flash black.
        let bounds = videoLayerView.bounds
        var image: UIImage?
        if let pixelBuffer = copyDisplayedPixelBufferIfAvailable() {
            freezeOverlaySignature = pixelBufferSignature(pixelBuffer)
            let ciImage = CIImage(cvPixelBuffer: pixelBuffer)
            if let cgImage = Self.ciContext.createCGImage(ciImage, from: ciImage.extent) {
                image = UIImage(cgImage: cgImage)
            }
        }
        if image == nil {
            let renderer = UIGraphicsImageRenderer(size: bounds.size)
            image = renderer.image { _ in
                // `drawHierarchy` captures most layer-backed content (including AVSampleBufferDisplayLayer)
                // better than `layer.render` on some OS versions.
                if !videoLayerView.drawHierarchy(in: bounds, afterScreenUpdates: false) {
                    videoLayerView.layer.render(in: UIGraphicsGetCurrentContext()!)
                }
            }
        }
        guard let image else { return }

        let overlay = UIImageView(image: image)
        overlay.frame = videoLayerView.frame
        overlay.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        overlay.contentMode = .scaleAspectFit
        overlay.clipsToBounds = true
        overlay.isUserInteractionEnabled = false
        containerView.insertSubview(overlay, aboveSubview: videoLayerView)
        freezeOverlayView = overlay
        freezeOverlayShownAt = CFAbsoluteTimeGetCurrent()
        freezeOverlayEnqueueCount = decodeQueue.sync { self.videoEnqueueCount }

        os_log(
            "freeze overlay shown componentId=%{public}@ reason=%{public}@",
            log: log,
            type: .info,
            componentId,
            String(describing: reason)
        )
    }

    @MainActor
    private func hideFreezeOverlay() {
        freezeOverlayView?.removeFromSuperview()
        freezeOverlayView = nil
        freezeOverlayShownAt = 0
        freezeOverlayEnqueueCount = 0
        freezeOverlaySignature = nil
    }

    @MainActor
    private func maybeHideFreezeOverlay(now: CFAbsoluteTime, enqueueCount: UInt64, waitingForKeyframe: Bool) {
        guard freezeOverlayView != nil else { return }
        if waitingForKeyframe {
            return
        }
        // Prefer hiding only once the displayed frame actually changes; falling back to enqueue
        // count/timeout keeps the UX acceptable on OS versions where we can't sample the layer.
        if let baseline = freezeOverlaySignature,
           let pixelBuffer = copyDisplayedPixelBufferIfAvailable()
        {
            let current = pixelBufferSignature(pixelBuffer)
            if current != 0 && current != baseline && now - freezeOverlayShownAt > 0.1 {
                hideFreezeOverlay()
                return
            }
        }

        // Fallback: hide once we have resumed enqueuing frames for a bit, or after a timeout.
        let delta = enqueueCount > freezeOverlayEnqueueCount ? enqueueCount - freezeOverlayEnqueueCount : 0
        if delta >= 12 || (freezeOverlayShownAt > 0 && now - freezeOverlayShownAt > 3.0) {
            hideFreezeOverlay()
        }
    }

    private static let ciContext = CIContext(options: nil)

    @MainActor
    private func copyDisplayedPixelBufferIfAvailable() -> CVPixelBuffer? {
        let selector = NSSelectorFromString("copyDisplayedPixelBuffer")
        guard videoLayerView.displayLayer.responds(to: selector) else {
            return nil
        }
        guard let unmanaged = videoLayerView.displayLayer.perform(selector) else {
            return nil
        }
        let value = unmanaged.takeRetainedValue()
        return unsafeBitCast(value, to: CVPixelBuffer.self)
    }

    @MainActor
    private func pixelBufferSignature(_ pixelBuffer: CVPixelBuffer) -> UInt64 {
        CVPixelBufferLockBaseAddress(pixelBuffer, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, .readOnly) }

        let planeCount = CVPixelBufferGetPlaneCount(pixelBuffer)
        let base: UnsafeMutableRawPointer?
        let bytesPerRow: Int
        let height: Int
        let width: Int
        if planeCount > 0 {
            base = CVPixelBufferGetBaseAddressOfPlane(pixelBuffer, 0)
            bytesPerRow = CVPixelBufferGetBytesPerRowOfPlane(pixelBuffer, 0)
            height = CVPixelBufferGetHeightOfPlane(pixelBuffer, 0)
            width = CVPixelBufferGetWidthOfPlane(pixelBuffer, 0)
        } else {
            base = CVPixelBufferGetBaseAddress(pixelBuffer)
            bytesPerRow = CVPixelBufferGetBytesPerRow(pixelBuffer)
            height = CVPixelBufferGetHeight(pixelBuffer)
            width = CVPixelBufferGetWidth(pixelBuffer)
        }
        guard let base else { return 0 }
        if bytesPerRow <= 0 || width <= 0 || height <= 0 { return 0 }

        let ptr = base.assumingMemoryBound(to: UInt8.self)
        var hash: UInt64 = 1469598103934665603 // FNV-1a offset basis
        let sampleX: [Int] = [0, width / 4, width / 2, (width * 3) / 4, max(0, width - 1)]
        let sampleY: [Int] = [0, height / 4, height / 2, (height * 3) / 4, max(0, height - 1)]
        for y in sampleY {
            let row = y * bytesPerRow
            for x in sampleX {
                let idx = row + min(x, bytesPerRow - 1)
                let b = ptr[idx]
                hash ^= UInt64(b)
                hash &*= 1099511628211
            }
        }
        return hash
    }

    @MainActor
    private func handleVideoDecodeFailure(infoDescription: String) {
        os_log(
            "displayLayer failed to decode componentId=%{public}@ info=%{public}@",
            log: log,
            type: .error,
            componentId,
            infoDescription
        )
        showFreezeOverlay(reason: "decode_failed")
        // Keep the last rendered frame visible during recovery to avoid a black flash.
        resetTimingForResume(keepLastFrame: true)
        playBeganAt = CFAbsoluteTimeGetCurrent()
        ComponentRouter.shared.emitComponentEvent(
            componentId: componentId,
            event: "waiting",
            detail: ["reason": "decode_failed"]
        )
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
        videoStuckTimer?.cancel()
        videoStuckTimer = nil
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
                ptsMs: normalizedPts,
                durationMs: 33
            ) else { return }
            self.lastEnqueuedVideoPtsMs = normalizedPts
            self.applyVideoSampleAttachments(sampleBuffer, isSync: true)
            videoLayerView.displayLayer.enqueue(sampleBuffer)
            self.maybeResyncTimebase(to: sampleBuffer.presentationTimeStamp)
            self.maybeNotifyPlay()
            if playRequested {
                if timelineResetPending {
                    timelineResetPending = false
                    renderSynchronizer.setRate(1.0, time: sampleBuffer.presentationTimeStamp)
                } else if renderSynchronizer.rate == 0.0 {
                    renderSynchronizer.rate = 1.0
                }
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
        let nalLengthSize = config.nalLengthSize ?? 4
        if config.format.lowercased() == "avcc" {
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
        ptsMs: UInt32,
        aacSamplesPerPacket: Int = 1024
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

        // Compressed audio (AAC) benefits from an explicit duration; some devices won't render
        // reliably with duration=invalid.
        let sampleRate = audioConfig?.sampleRate ?? 44100
        let samples = max(1, aacSamplesPerPacket)
        let duration = CMTime(value: CMTimeValue(samples), timescale: CMTimeScale(sampleRate))

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
        var timing = CMSampleTimingInfo(duration: duration, presentationTimeStamp: pts, decodeTimeStamp: dts)
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
            return nil
        }
        return sampleBuffer
    }

    @MainActor
    private func suppressNativePlayback(in view: UIView) {
        suppressedLayers = suppressedLayers.filter { $0.value != nil }

        let trackedLayers = Set(suppressedLayers.compactMap { $0.value }.map { ObjectIdentifier($0) })
        if let layers = view.layer.sublayers {
            for layer in layers where layer is AVPlayerLayer {
                layer.isHidden = true
                if !trackedLayers.contains(ObjectIdentifier(layer)) {
                    suppressedLayers.append(WeakRef(layer))
                }
            }
        }
    }

    @MainActor
    private func restoreNativePlayback() {
        for layer in suppressedLayers.compactMap({ $0.value }) {
            layer.isHidden = false
        }
        suppressedLayers.removeAll()
    }

    @MainActor
    private func handleAppWillResignActive() {
        wasPlayingBeforeBackground = playRequested || isPlaying || renderSynchronizer.rate != 0.0
        renderSynchronizer.rate = 0.0
        isPlaying = false
        decodeQueue.async { [weak self] in
            self?.audioRenderer.flush()
        }
        stopPcmAudioEngine()
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
            playRequested = true
            if waitingForVideoKeyframe {
                resumeAfterVideoKeyframe = true
                gateAudioUntilVideo = true
            }
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
        let shouldUpdateRate = sampleRate != nil && sampleRate != audioSessionSampleRate
        let session = AVAudioSession.sharedInstance()
        do {
            if let rate = sampleRate, shouldUpdateRate {
                try? session.setPreferredSampleRate(Double(rate))
                audioSessionSampleRate = rate
            }
            if !audioSessionConfigured {
                try session.setCategory(.playback, mode: .default, options: [])
                audioSessionConfigured = true
            }
            // Re-activate on demand; some devices deactivate the session after stream pause/resume.
            try session.setActive(true)
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
        let format = config.format.lowercased()
        let nalLengthSize = config.nalLengthSize ?? 4

        // For iOS streams we expect AVCC (length-prefixed NALs). Avoid sniffing for AnnexB start
        // codes here: an AVCC length prefix like `00 00 01 xx` is valid and can be misdetected as
        // AnnexB, corrupting subsequent frames and resulting in "first frame only" playback.
        if format == "annexb" {
            return annexBToAvcc(data: data, nalLengthSize: nalLengthSize)
        }
        if format == "avcc" {
            return data
        }
        return data
    }

    private func isValidNalHeader(codec: String, header: UInt8) -> Bool {
        if codec == "h265" || codec == "hevc" {
            let nalType = Int((header >> 1) & 0x3F)
            return nalType <= 47
        }
        let nalType = Int(header & 0x1F)
        return (1...23).contains(nalType)
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
        ptsMs: UInt32,
        durationMs: UInt32
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
        // For H.264/H.265 samples enqueued into AVSampleBufferDisplayLayer, providing a DTS can
        // cause some streams to stall (especially when the upstream source does not provide a
        // stable decode timeline). Use an invalid DTS and let PTS drive scheduling.
        let dts = CMTime.invalid
        let duration = CMTime(value: CMTimeValue(max(1, durationMs)), timescale: 1000)
        var timing = CMSampleTimingInfo(duration: duration, presentationTimeStamp: pts, decodeTimeStamp: dts)
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

private struct AudioConfig: Equatable {
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
