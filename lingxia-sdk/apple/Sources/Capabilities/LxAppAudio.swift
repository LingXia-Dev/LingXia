import Foundation
@preconcurrency import AVFoundation
import CLingXiaRustAPI
#if os(iOS)
import UIKit
#elseif os(macOS)
import CoreAudio
#endif

/// Native backend of the host audio coordinator. Rust decides focus, queue,
/// and state; this applies commands and reports what the platform did.
/// Everything runs on the main thread, in the order commands were sent.
enum LxAppAudio {
    private static let capEarpiece: UInt32 = 1
    private static let capProximity: UInt32 = 2

    nonisolated static func capabilities() -> UInt32 {
        #if os(iOS)
        // Earpiece and proximity sensor come with the phone form factor.
        var size = 0
        sysctlbyname("hw.machine", nil, &size, nil, 0)
        var machine = [CChar](repeating: 0, count: max(size, 1))
        sysctlbyname("hw.machine", &machine, &size, nil, 0)
        return String(cString: machine).hasPrefix("iPhone") ? capEarpiece | capProximity : 0
        #else
        return 0
        #endif
    }

    nonisolated static func attach(callback_id: UInt64) {
        onMain { AudioBackend.shared.attach(callback_id) }
    }

    nonisolated static func send(command_json: RustStr) {
        let json = command_json.toString()
        onMain { AudioBackend.shared.handle(json) }
    }

    /// LxVideo playback events; audible output is a coordinator participant.
    @MainActor
    static func videoEvent(componentId: String, event: String, detail: [String: Any]) {
        AudioBackend.shared.videoEvent(componentId: componentId, event: event, detail: detail)
    }

    @MainActor
    static func videoGone(componentId: String) {
        AudioBackend.shared.videoEvent(componentId: componentId, event: "stop", detail: [:])
        AudioBackend.shared.forgetVideo(componentId)
    }
}

/// Run on the main actor: inline when already there (keeps command order),
/// otherwise queued behind earlier work.
private func onMain(_ block: @escaping @MainActor @Sendable () -> Void) {
    if Thread.isMainThread {
        MainActor.assumeIsolated(block)
    } else {
        DispatchQueue.main.async { MainActor.assumeIsolated(block) }
    }
}

private func milliseconds(_ time: CMTime) -> Int64? {
    guard time.isNumeric, !time.isIndefinite else { return nil }
    let seconds = CMTimeGetSeconds(time)
    return seconds.isFinite ? max(0, Int64(seconds * 1000)) : nil
}

private func cmTime(_ ms: Int64) -> CMTime {
    CMTime(value: ms, timescale: 1000)
}

@MainActor
private final class AudioBackend {
    static let shared = AudioBackend()

    private var callbackId: UInt64 = 0
    private var players: [UInt64: Player] = [:]
    private var sounds: [UInt64: SoundBank] = [:]
    private let session = Session()
    private var videos: [String: (playing: Bool, silent: Bool)] = [:]

    func attach(_ id: UInt64) {
        callbackId = id
        session.observeRoutes()
    }

    func emit(_ event: [String: Any]) {
        guard callbackId != 0,
              let data = try? JSONSerialization.data(withJSONObject: event),
              let json = String(data: data, encoding: .utf8)
        else { return }
        _ = onCallback(callbackId, true, json)
    }

    func handle(_ json: String) {
        guard let data = json.data(using: .utf8),
              let command = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
              let op = command["op"] as? String
        else {
            _ = LXLog.warn("audio command unreadable: \(json)", category: "Audio")
            return
        }
        let id = (command["id"] as? NSNumber)?.uint64Value ?? 0
        let number = { (key: String) in (command[key] as? NSNumber) }
        switch op {
        case "player.create":
            players[id]?.release()
            players[id] = Player(id: id, tickMs: number("tickMs")?.intValue ?? 250)
        case "player.load":
            players[id]?.load(
                req: number("req")?.uint64Value ?? 0,
                uri: command["uri"] as? String ?? "",
                headers: command["headers"] as? [String: String] ?? [:],
                startMs: number("startMs")?.int64Value ?? 0
            )
        case "player.play": players[id]?.play()
        case "player.pause": players[id]?.pause()
        case "player.seek":
            players[id]?.seek(req: number("req")?.uint64Value ?? 0, positionMs: number("positionMs")?.int64Value ?? 0)
        case "player.unload": players[id]?.unload()
        case "player.volume": players[id]?.setVolume(number("volume")?.floatValue ?? 1)
        case "player.rate": players[id]?.setRate(number("rate")?.floatValue ?? 1)
        case "player.dispose": players.removeValue(forKey: id)?.release()
        case "sound.load":
            sounds.removeValue(forKey: id)?.release()
            let bank = SoundBank(id: id)
            sounds[id] = bank
            bank.load(req: number("req")?.uint64Value ?? 0, path: command["path"] as? String ?? "")
        case "sound.play":
            sounds[id]?.play(
                voice: number("voice")?.uint64Value ?? 0,
                volume: number("volume")?.floatValue ?? 1,
                rate: number("rate")?.floatValue ?? 1,
                pan: number("pan")?.floatValue ?? 0,
                loop: command["loop"] as? Bool ?? false
            )
        case "sound.stop": sounds[id]?.stop(voice: number("voice")?.uint64Value ?? 0)
        case "sound.dispose": sounds.removeValue(forKey: id)?.release()
        case "session":
            session.apply(
                active: command["active"] as? Bool ?? false,
                focus: command["focus"] as? String,
                obeySilentSwitch: command["obeySilentSwitch"] as? Bool ?? false,
                earpiece: (command["output"] as? String) == "earpiece",
                proximity: command["proximity"] as? Bool ?? false
            )
        case "video.pause":
            guard let componentId = command["componentId"] as? String else { return }
            #if os(iOS)
            _ = ComponentRouter.shared.dispatchCommand(componentId: componentId, name: "pause", params: nil)
            #else
            _ = MacComponentRouter.shared.dispatchCommand(componentId: componentId, name: "pause", params: nil)
            #endif
        default:
            _ = LXLog.warn("unknown audio op \(op)", category: "Audio")
        }
    }

    func videoEvent(componentId: String, event: String, detail: [String: Any]) {
        var state = videos[componentId] ?? (false, false)
        let before = state.playing && !state.silent
        switch event {
        case "playing":
            state.playing = true
        case "pause", "stop", "ended", "error":
            state.playing = false
        case "volumechange":
            let muted = detail["muted"] as? Bool ?? false
            let volume = (detail["volume"] as? NSNumber)?.doubleValue ?? 1
            state.silent = muted || volume <= 0
        default:
            return
        }
        videos[componentId] = state
        let after = state.playing && !state.silent
        if before != after {
            emit(["event": "video", "componentId": componentId, "playing": after])
        }
    }

    func forgetVideo(_ componentId: String) {
        videos.removeValue(forKey: componentId)
    }
}

// MARK: - Player

@MainActor
private final class Player {
    private let id: UInt64
    private let tickMs: Int
    private let player = AVPlayer()
    private var loadReq: UInt64?
    private var seekReq: UInt64?
    private var rate: Float = 1
    private var remote = false
    /// Play was asked for and not yet countermanded.
    private var wantsPlay = false
    private var reportedPlaying = false
    private var buffering = false
    private var ended = false
    private var itemStatus: NSKeyValueObservation?
    private var controlStatus: NSKeyValueObservation?
    private var itemObservers: [NSObjectProtocol] = []
    private var ticker: Timer?

    init(id: UInt64, tickMs: Int) {
        self.id = id
        self.tickMs = max(tickMs, 50)
        player.automaticallyWaitsToMinimizeStalling = true
        controlStatus = player.observe(\.timeControlStatus, options: [.new]) { [weak self] _, _ in
            onMain { self?.controlStatusChanged() }
        }
    }

    private func emit(_ name: String, _ fields: [String: Any] = [:]) {
        var event = fields
        event["event"] = name
        event["id"] = id
        AudioBackend.shared.emit(event)
    }

    private var positionMs: Int64 { milliseconds(player.currentTime()) ?? 0 }

    private var durationMs: Int64? {
        guard let item = player.currentItem, let ms = milliseconds(item.duration), ms > 0 else { return nil }
        return ms
    }

    func load(req: UInt64, uri: String, headers: [String: String], startMs: Int64) {
        detachItem()
        player.pause()
        loadReq = req
        let lower = uri.lowercased()
        remote = lower.hasPrefix("http://") || lower.hasPrefix("https://")
        let url: URL
        if remote {
            guard let parsed = URL(string: uri) else {
                fail(req: req, code: "invalid_source", message: "unparsable URL")
                return
            }
            url = parsed
        } else {
            guard FileManager.default.fileExists(atPath: uri) else {
                fail(req: req, code: "source_not_found", message: "no file at \(uri)")
                return
            }
            url = URL(fileURLWithPath: uri)
        }
        var options: [String: Any] = [:]
        if !headers.isEmpty {
            options["AVURLAssetHTTPHeaderFieldsKey"] = headers
        }
        let item = AVPlayerItem(asset: AVURLAsset(url: url, options: options))
        item.audioTimePitchAlgorithm = .spectral
        itemStatus = item.observe(\.status, options: [.new]) { [weak self] item, _ in
            onMain { self?.itemStatusChanged(item, req: req, startMs: startMs) }
        }
        let center = NotificationCenter.default
        itemObservers = [
            center.addObserver(forName: AVPlayerItem.didPlayToEndTimeNotification, object: item, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.didEnd() }
            },
            center.addObserver(forName: AVPlayerItem.failedToPlayToEndTimeNotification, object: item, queue: .main) { [weak self] note in
                let error = note.userInfo?[AVPlayerItemFailedToPlayToEndTimeErrorKey] as? NSError
                MainActor.assumeIsolated { self?.playbackFailed(error) }
            },
        ]
        player.replaceCurrentItem(with: item)
    }

    private func itemStatusChanged(_ item: AVPlayerItem, req: UInt64, startMs: Int64) {
        guard item === player.currentItem, loadReq == req else { return }
        switch item.status {
        case .readyToPlay:
            if startMs > 0 {
                player.seek(to: cmTime(startMs), toleranceBefore: .zero, toleranceAfter: .zero) { [weak self] _ in
                    onMain { self?.loaded(req: req) }
                }
            } else {
                loaded(req: req)
            }
        case .failed:
            let error = item.error as NSError?
            fail(req: req, code: errorCode(error), message: error?.localizedDescription ?? "load failed")
        default:
            break
        }
    }

    private func loaded(req: UInt64) {
        guard loadReq == req else { return }
        loadReq = nil
        emit("player.loaded", ["req": req, "durationMs": durationMs.map { $0 as Any } ?? NSNull()])
    }

    private func fail(req: UInt64?, code: String, message: String) {
        stopTicker()
        var fields: [String: Any] = ["code": code, "message": message]
        if let req { fields["req"] = req }
        loadReq = nil
        wantsPlay = false
        reportedPlaying = false
        emit("player.error", fields)
    }

    private func playbackFailed(_ error: NSError?) {
        fail(req: loadReq, code: errorCode(error), message: error?.localizedDescription ?? "playback failed")
    }

    func play() {
        wantsPlay = true
        ended = false
        player.rate = rate
    }

    func pause() {
        wantsPlay = false
        player.pause()
    }

    func seek(req: UInt64, positionMs target: Int64) {
        seekReq = req
        ended = false
        player.seek(to: cmTime(target), toleranceBefore: .zero, toleranceAfter: .zero) { [weak self] _ in
            onMain {
                guard let self, self.seekReq == req else { return }
                self.seekReq = nil
                self.emit("player.seeked", ["req": req, "positionMs": self.positionMs])
            }
        }
    }

    func unload() {
        detachItem()
        player.pause()
        player.replaceCurrentItem(with: nil)
    }

    func setVolume(_ volume: Float) {
        player.volume = min(max(volume, 0), 1)
    }

    func setRate(_ value: Float) {
        rate = value
        if player.rate != 0 {
            player.rate = value
        }
    }

    func release() {
        unload()
        controlStatus?.invalidate()
        controlStatus = nil
    }

    /// Forget the current item without reporting the pause that follows.
    private func detachItem() {
        stopTicker()
        itemStatus?.invalidate()
        itemStatus = nil
        itemObservers.forEach(NotificationCenter.default.removeObserver)
        itemObservers = []
        loadReq = nil
        seekReq = nil
        wantsPlay = false
        reportedPlaying = false
        buffering = false
        ended = false
    }

    private func didEnd() {
        ended = true
        wantsPlay = false
        reportedPlaying = false
        stopTicker()
        emit("player.ended")
    }

    private func controlStatusChanged() {
        switch player.timeControlStatus {
        case .playing:
            if buffering {
                buffering = false
                emit("player.buffering", ["buffering": false])
            }
            if !reportedPlaying {
                reportedPlaying = true
                emit("player.playing", ["positionMs": positionMs])
            }
            startTicker()
        case .waitingToPlayAtSpecifiedRate:
            if wantsPlay && loadReq == nil && !buffering {
                buffering = true
                emit("player.buffering", ["buffering": true])
            }
        case .paused:
            stopTicker()
            let wasActive = reportedPlaying || wantsPlay
            reportedPlaying = false
            wantsPlay = false
            if buffering {
                buffering = false
                emit("player.buffering", ["buffering": false])
            }
            // Reaching the end pauses too, sometimes before the end
            // notification: that is `ended`, not a pause.
            if ended || atEnd() || !wasActive { return }
            emit("player.paused", ["positionMs": positionMs])
        @unknown default:
            break
        }
    }

    private func atEnd() -> Bool {
        guard let duration = durationMs else { return false }
        return positionMs >= duration - 50
    }

    private func startTicker() {
        guard ticker == nil else { return }
        let timer = Timer(timeInterval: Double(tickMs) / 1000, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.tick() }
        }
        RunLoop.main.add(timer, forMode: .common)
        ticker = timer
    }

    private func stopTicker() {
        ticker?.invalidate()
        ticker = nil
    }

    private func tick() {
        var fields: [String: Any] = ["positionMs": positionMs]
        if let item = player.currentItem {
            let now = player.currentTime()
            let buffered = item.loadedTimeRanges
                .map(\.timeRangeValue)
                .first { CMTimeRangeContainsTime($0, time: now) }
                .flatMap { milliseconds(CMTimeRangeGetEnd($0)) }
            if let buffered { fields["bufferedMs"] = buffered }
        }
        if let duration = durationMs { fields["durationMs"] = duration }
        emit("player.progress", fields)
    }

    private func errorCode(_ error: NSError?) -> String {
        var current = error
        while let e = current {
            switch e.domain {
            case NSURLErrorDomain:
                return e.code == NSURLErrorFileDoesNotExist ? "source_not_found" : "network"
            case "CoreMediaErrorDomain" where e.code == -12938:
                return "source_not_found" // HTTP 404
            case "CoreMediaErrorDomain" where e.code == -12660 || e.code == -12937:
                return "network" // HTTP 403 / other HTTP failure
            // Not recognized, failed to parse, decoder not found, unsupported.
            case AVFoundationErrorDomain where [-11828, -11829, -11833, -11821].contains(e.code):
                return "unsupported_format"
            default:
                break
            }
            current = e.userInfo[NSUnderlyingErrorKey] as? NSError
        }
        return remote ? "decode_failed" : "unsupported_format"
    }
}

// MARK: - Sounds

/// One engine mixes every voice of every sound.
@MainActor
private final class SoundEngine {
    static let shared = SoundEngine()
    let engine = AVAudioEngine()
    private var configObserver: NSObjectProtocol?

    private init() {
        configObserver = NotificationCenter.default.addObserver(
            forName: .AVAudioEngineConfigurationChange, object: engine, queue: .main
        ) { _ in
            MainActor.assumeIsolated { SoundEngine.shared.restartIfNeeded() }
        }
    }

    func running() -> AVAudioEngine? {
        _ = engine.mainMixerNode
        if !engine.isRunning {
            do {
                try engine.start()
            } catch {
                _ = LXLog.warn("audio engine failed to start", category: "Audio", error: error)
                return nil
            }
        }
        return engine
    }

    /// A route change stops the engine; voices still attached keep going.
    private func restartIfNeeded() {
        if !engine.attachedNodes.contains(where: { $0 is AVAudioPlayerNode }) { return }
        try? engine.start()
    }
}

@MainActor
private final class SoundBank {
    private let id: UInt64
    private var buffer: AVAudioPCMBuffer?
    private var voices: [UInt64: (node: AVAudioPlayerNode, speed: AVAudioUnitVarispeed)] = [:]

    init(id: UInt64) {
        self.id = id
    }

    private func emit(_ name: String, _ fields: [String: Any]) {
        var event = fields
        event["event"] = name
        event["id"] = id
        AudioBackend.shared.emit(event)
    }

    func load(req: UInt64, path: String) {
        guard FileManager.default.fileExists(atPath: path) else {
            emit("sound.error", ["req": req, "code": "source_not_found", "message": "no file at \(path)"])
            return
        }
        do {
            let file = try AVAudioFile(forReading: URL(fileURLWithPath: path))
            guard file.length > 0,
                  let pcm = AVAudioPCMBuffer(pcmFormat: file.processingFormat, frameCapacity: AVAudioFrameCount(file.length))
            else {
                emit("sound.error", ["req": req, "code": "decode_failed", "message": "empty audio file"])
                return
            }
            try file.read(into: pcm)
            buffer = pcm
            let durationMs = Int64(Double(pcm.frameLength) / pcm.format.sampleRate * 1000)
            emit("sound.loaded", ["req": req, "durationMs": durationMs])
        } catch {
            emit("sound.error", ["req": req, "code": "unsupported_format", "message": error.localizedDescription])
        }
    }

    func play(voice: UInt64, volume: Float, rate: Float, pan: Float, loop: Bool) {
        guard let buffer, let engine = SoundEngine.shared.running() else {
            emit("sound.ended", ["voice": voice])
            return
        }
        let node = AVAudioPlayerNode()
        let speed = AVAudioUnitVarispeed()
        engine.attach(node)
        engine.attach(speed)
        engine.connect(node, to: speed, format: buffer.format)
        engine.connect(speed, to: engine.mainMixerNode, format: buffer.format)
        speed.rate = min(max(rate, 0.5), 2)
        node.volume = min(max(volume, 0), 1)
        node.pan = min(max(pan, -1), 1)
        voices[voice] = (node, speed)
        let token = ObjectIdentifier(node)
        node.scheduleBuffer(buffer, at: nil, options: loop ? .loops : [], completionCallbackType: .dataPlayedBack) { [weak self] _ in
            onMain { self?.finished(voice: voice, token: token) }
        }
        node.play()
    }

    private func finished(voice: UInt64, token: ObjectIdentifier) {
        guard let entry = voices[voice], ObjectIdentifier(entry.node) == token else { return }
        voices.removeValue(forKey: voice)
        detach(entry)
        emit("sound.ended", ["voice": voice])
    }

    func stop(voice: UInt64) {
        guard let entry = voices.removeValue(forKey: voice) else { return }
        entry.node.stop()
        detach(entry)
    }

    func release() {
        let all = voices.values
        voices.removeAll()
        for entry in all {
            entry.node.stop()
            detach(entry)
        }
        buffer = nil
    }

    private func detach(_ entry: (node: AVAudioPlayerNode, speed: AVAudioUnitVarispeed)) {
        let engine = SoundEngine.shared.engine
        engine.detach(entry.node)
        engine.detach(entry.speed)
    }
}

// MARK: - Session and routes

@MainActor
private final class Session {
    private var activated = false
    private var earpiece = false
    private var observing = false
    private var lastRoute = ""
    private var proximityObserver: NSObjectProtocol?
    private var near = false

    private func emit(_ event: [String: Any]) {
        AudioBackend.shared.emit(event)
    }

    func apply(active: Bool, focus: String?, obeySilentSwitch: Bool, earpiece wantsEarpiece: Bool, proximity: Bool) {
        #if os(iOS)
        let session = AVAudioSession.sharedInstance()
        let routeEarpiece = active && wantsEarpiece
        if active {
            let category: AVAudioSession.Category
            var options: AVAudioSession.CategoryOptions = []
            if routeEarpiece {
                // `.playAndRecord` routes to the receiver unless told otherwise.
                category = .playAndRecord
                options = [.allowBluetoothA2DP]
            } else if obeySilentSwitch {
                category = focus == "exclusive" ? .soloAmbient : .ambient
            } else {
                category = .playback
                if focus == "duck" { options = [.duckOthers] }
                if focus == "mix" { options = [.mixWithOthers] }
            }
            do {
                try session.setCategory(category, mode: .default, options: options)
                if routeEarpiece { try session.overrideOutputAudioPort(.none) }
                try session.setActive(true)
                activated = true
            } catch {
                _ = LXLog.warn("audio session update failed", category: "Audio", error: error)
            }
        } else if activated {
            activated = false
            try? session.setActive(false, options: .notifyOthersOnDeactivation)
        }
        if routeEarpiece != earpiece {
            earpiece = routeEarpiece
            emitRoute(reason: "override", lost: false, force: true)
        }
        setProximity(active && proximity)
        #endif
    }

    #if os(iOS)
    private func setProximity(_ enabled: Bool) {
        let device = UIDevice.current
        if enabled == device.isProximityMonitoringEnabled { return }
        device.isProximityMonitoringEnabled = enabled
        if enabled, device.isProximityMonitoringEnabled {
            proximityObserver = NotificationCenter.default.addObserver(
                forName: UIDevice.proximityStateDidChangeNotification, object: device, queue: .main
            ) { [weak self] _ in
                MainActor.assumeIsolated { self?.proximityChanged() }
            }
        } else if !enabled {
            if let observer = proximityObserver {
                NotificationCenter.default.removeObserver(observer)
            }
            proximityObserver = nil
            if near {
                near = false
                emit(["event": "proximity", "near": false])
            }
        }
    }

    private func proximityChanged() {
        let isNear = UIDevice.current.proximityState
        if isNear != near {
            near = isNear
            emit(["event": "proximity", "near": isNear])
        }
    }
    #endif

    func observeRoutes() {
        if observing {
            emitRoute(reason: "other", lost: false, force: true)
            return
        }
        observing = true
        #if os(iOS)
        let center = NotificationCenter.default
        let session = AVAudioSession.sharedInstance()
        center.addObserver(forName: AVAudioSession.routeChangeNotification, object: session, queue: .main) { [weak self] note in
            let raw = note.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt
            MainActor.assumeIsolated { self?.routeChanged(raw.flatMap(AVAudioSession.RouteChangeReason.init)) }
        }
        center.addObserver(forName: AVAudioSession.interruptionNotification, object: session, queue: .main) { [weak self] note in
            let info = note.userInfo ?? [:]
            let type = (info[AVAudioSessionInterruptionTypeKey] as? UInt).flatMap(AVAudioSession.InterruptionType.init)
            let options = (info[AVAudioSessionInterruptionOptionKey] as? UInt).map(AVAudioSession.InterruptionOptions.init) ?? []
            let suspended = (info[AVAudioSessionInterruptionReasonKey] as? UInt) == AVAudioSession.InterruptionReason.appWasSuspended.rawValue
            MainActor.assumeIsolated {
                self?.interrupted(type: type, shouldResume: options.contains(.shouldResume), appWasSuspended: suspended)
            }
        }
        #else
        observeDefaultOutput()
        #endif
        emitRoute(reason: "other", lost: false, force: true)
    }

    #if os(iOS)
    private func routeChanged(_ reason: AVAudioSession.RouteChangeReason?) {
        switch reason {
        case .newDeviceAvailable: emitRoute(reason: "deviceAdded", lost: false)
        case .oldDeviceUnavailable: emitRoute(reason: "deviceRemoved", lost: true, force: true)
        case .override: emitRoute(reason: "override", lost: false)
        default: emitRoute(reason: "other", lost: false)
        }
    }

    private func interrupted(type: AVAudioSession.InterruptionType?, shouldResume: Bool, appWasSuspended: Bool) {
        switch type {
        case .began:
            // A suspension notice arrives late, after the app is back; the
            // hide already paused everything.
            if appWasSuspended { return }
            emit(["event": "interruption", "phase": "began", "reason": "otherApp", "shouldResume": false])
        case .ended:
            emit(["event": "interruption", "phase": "ended", "reason": "otherApp", "shouldResume": shouldResume])
        default:
            break
        }
    }
    #endif

    private func emitRoute(reason: String, lost: Bool, force: Bool = false) {
        let (output, name) = currentRoute()
        let key = "\(output)|\(name ?? "")"
        if !force && key == lastRoute { return }
        lastRoute = key
        var event: [String: Any] = ["event": "route", "output": output, "reason": reason, "lost": lost]
        if let name { event["name"] = name }
        emit(event)
    }

    private func currentRoute() -> (String, String?) {
        #if os(iOS)
        guard let port = AVAudioSession.sharedInstance().currentRoute.outputs.first else {
            return (earpiece ? "earpiece" : "speaker", nil)
        }
        let name = port.portName.isEmpty ? nil : port.portName
        switch port.portType {
        case .builtInSpeaker: return ("speaker", nil)
        case .builtInReceiver: return ("earpiece", nil)
        case .headphones, .headsetMic: return ("headphones", name)
        case .bluetoothA2DP, .bluetoothLE, .bluetoothHFP: return ("bluetooth", name)
        case .usbAudio: return ("usb", name)
        case .HDMI: return ("hdmi", name)
        case .airPlay: return ("airplay", name)
        default: return ("other", name)
        }
        #else
        return MacOutput.current()
        #endif
    }

    #if os(macOS)
    private var watchedDevice: AudioDeviceID = 0

    private func observeDefaultOutput() {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDefaultOutputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        AudioObjectAddPropertyListenerBlock(AudioObjectID(kAudioObjectSystemObject), &address, .main) { [weak self] _, _ in
            MainActor.assumeIsolated { self?.outputChanged() }
        }
        watchDataSource()
    }

    /// Headphones on the built-in device show up as a data source change.
    private func watchDataSource() {
        let device = MacOutput.defaultDevice()
        guard device != 0, device != watchedDevice else { return }
        watchedDevice = device
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioDevicePropertyDataSource,
            mScope: kAudioDevicePropertyScopeOutput,
            mElement: kAudioObjectPropertyElementMain
        )
        AudioObjectAddPropertyListenerBlock(device, &address, .main) { [weak self] _, _ in
            MainActor.assumeIsolated { self?.outputChanged() }
        }
    }

    private func outputChanged() {
        let previous = lastRoute.split(separator: "|").first.map(String.init) ?? "speaker"
        let (output, _) = MacOutput.current()
        let external: Set<String> = ["headphones", "bluetooth", "usb", "hdmi", "airplay"]
        let lost = external.contains(previous) && !external.contains(output)
        watchDataSource()
        emitRoute(reason: lost ? "deviceRemoved" : (external.contains(output) ? "deviceAdded" : "other"), lost: lost)
    }
    #endif
}

#if os(macOS)
private enum MacOutput {
    static func defaultDevice() -> AudioDeviceID {
        var device = AudioDeviceID(0)
        var size = UInt32(MemoryLayout<AudioDeviceID>.size)
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDefaultOutputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        let status = AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, &device)
        return status == noErr ? device : 0
    }

    static func current() -> (String, String?) {
        let device = defaultDevice()
        guard device != 0 else { return ("speaker", nil) }
        var transport: UInt32 = 0
        var size = UInt32(MemoryLayout<UInt32>.size)
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioDevicePropertyTransportType,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        AudioObjectGetPropertyData(device, &address, 0, nil, &size, &transport)

        var cfName: Unmanaged<CFString>?
        size = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
        address.mSelector = kAudioObjectPropertyName
        AudioObjectGetPropertyData(device, &address, 0, nil, &size, &cfName)
        let name = cfName?.takeRetainedValue() as String?

        switch transport {
        case kAudioDeviceTransportTypeBuiltIn:
            var source: UInt32 = 0
            size = UInt32(MemoryLayout<UInt32>.size)
            address.mSelector = kAudioDevicePropertyDataSource
            address.mScope = kAudioDevicePropertyScopeOutput
            AudioObjectGetPropertyData(device, &address, 0, nil, &size, &source)
            return source == 0x6864_706E /* 'hdpn' */ ? ("headphones", name) : ("speaker", nil)
        case kAudioDeviceTransportTypeBluetooth, kAudioDeviceTransportTypeBluetoothLE:
            return ("bluetooth", name)
        case kAudioDeviceTransportTypeUSB:
            return ("usb", name)
        case kAudioDeviceTransportTypeHDMI, kAudioDeviceTransportTypeDisplayPort:
            return ("hdmi", name)
        case kAudioDeviceTransportTypeAirPlay:
            return ("airplay", name)
        default:
            return ("other", name)
        }
    }
}
#endif
