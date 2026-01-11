import Foundation
import UIKit
import OSLog
import CLingXiaRustAPI

#if os(iOS)

@MainActor
final class VideoComponentFactory: LxNativeComponentFactory {
    func make(
        id: String,
        initialProps: [String: Any],
        eventSink: @escaping ([String: Any]) -> Void
    ) -> LxNativeComponent {
        VideoComponent(id: id, initialProps: initialProps, eventSink: eventSink)
    }
}

@MainActor
final class VideoComponent: NSObject, LxNativeComponent {
    let id: String
    let view: UIView

    private let player: LxMediaPlayer
    private var lastPropsVolume: Double?
    private var lastPropsMuted: Bool?

    init(id: String, initialProps: [String: Any], eventSink: @escaping ([String: Any]) -> Void) {
        self.id = id
        self.player = LxMediaPlayer(eventSink: eventSink)
        self.view = player.view
        super.init()
        lastPropsVolume = Self.double(from: initialProps["volume"])
        lastPropsMuted = Self.bool(from: initialProps["muted"])
        player.update(config: Self.makeConfig(from: initialProps))
    }

    func mount(in host: UIView) {
        player.attach(to: host)
    }

    func update(props: [String: Any]) {
        var config = Self.makeConfig(from: props)

        let nextVolume = Self.double(from: props["volume"])
        if let nextVolume {
            if let lastVolume = lastPropsVolume, abs(nextVolume - lastVolume) < 0.000_1 {
                config.volume = nil
            } else {
                lastPropsVolume = nextVolume
            }
        } else {
            config.volume = nil
        }

        let nextMuted = Self.bool(from: props["muted"])
        if nextMuted == nil || nextMuted == lastPropsMuted {
            config.muted = nil
        } else {
            lastPropsMuted = nextMuted
        }

        player.update(config: config)
    }

    func setFrame(_ frame: CGRect) {
        player.setFrame(frame)
    }

    func focus() { }
    func blur() { }

    func handleCommand(name: String, params: [String: Any]?) {
        if name == "setDuration" {
            let duration = Self.double(from: params?["duration"])
            player.setExternalDurationSeconds(duration)
            return
        }
        if name == "notifyEnded" {
            player.handleStreamDecoderEvent("ended")
            return
        }
        guard let command = Self.makeCommand(name: name, params: params) else {
            return
        }
        player.handle(command: command)
    }

    func handleStreamDecoderEvent(_ event: String) {
        player.handleStreamDecoderEvent(event)
    }

    func setStreamDecoderActive(_ active: Bool) {
        if active {
            player.setStreamDecoderActive(true, componentId: id) { [weak self] name, params in
                guard let self = self else { return false }
                return StreamDecoderRegistry.shared.handleCommand(
                    componentId: self.id,
                    name: name,
                    params: params
                )
            }
        } else {
            player.setStreamDecoderActive(false, componentId: nil, commandHandler: nil)
        }
    }

    func unmount() {
        player.detach()
    }

    // MARK: - Helpers
    private static func url(from string: String) -> URL? {
        let raw = string.trimmingCharacters(in: .whitespacesAndNewlines)
        if raw.isEmpty { return nil }

        // Keep remote URLs as-is.
        if raw.hasPrefix("http://") || raw.hasPrefix("https://") {
            return URL(string: raw)
        }

        let current = getCurrentLxApp()
        let appId = current.appid.toString()
        let resolved = resolveLxUri(appId, raw)?.toString() ?? raw

        if let url = URL(string: resolved), url.scheme != nil {
            return url
        }
        if resolved.hasPrefix("/") {
            return URL(fileURLWithPath: resolved)
        }
        return nil
    }

    private static func double(from value: Any?) -> Double? {
        if let value = value as? Double { return value }
        if let value = value as? Float { return Double(value) }
        if let value = value as? Int { return Double(value) }
        if let value = value as? NSNumber { return value.doubleValue }
        return nil
    }

    private static func bool(from value: Any?) -> Bool? {
        if let value = value as? Bool { return value }
        if let value = value as? NSNumber { return value.boolValue }
        return nil
    }

    private enum CommandName: String {
        case play
        case pause
        case stop
        case seek
        case setVolume
        case setMuted
        case setPlaybackRate
        case enterFullscreen
        case exitFullscreen
    }

    private static func makeConfig(from props: [String: Any]) -> LxMediaPlayerConfig {
        var config = LxMediaPlayerConfig()

        if let source = props["source"] as? [String: Any],
           let type = source["type"] as? String,
           let value = source["value"] as? String {
            switch type {
            case "url":
                if let url = url(from: value) {
                    config.source = .url(url)
                }
            case "file":
                if let url = url(from: value) {
                    if url.isFileURL {
                        config.source = .file(path: url.path)
                    } else {
                        config.source = .url(url)
                    }
                } else {
                    config.source = .file(path: value)
                }
            default:
                break
            }
        }

        if config.source == nil, let srcString = props["src"] as? String, let url = url(from: srcString) {
            config.src = url
        }

        if let poster = props["poster"] as? String, let url = url(from: poster) {
            config.poster = url
        }

        // Duration (playback segment) - seconds preferred; also accept milliseconds.
        if let duration = Self.double(from: props["duration"]), duration > 0 {
            config.duration = duration
        } else if let durationMs = Self.double(from: props["durationMs"]), durationMs > 0 {
            config.duration = durationMs / 1000.0
        }

        if let autoplay = props["autoplay"] as? Bool {
            config.autoplay = autoplay
        }
        if let loop = props["loop"] as? Bool {
            config.loop = loop
        }
        if let muted = Self.bool(from: props["muted"]) {
            config.muted = muted
        }
        if let volume = Self.double(from: props["volume"]) {
            config.volume = volume
        }
        if let controls = props["controls"] as? Bool {
            config.controls = controls
        }
        if let progressBar = props["progressBar"] as? Bool {
            config.progressBar = progressBar
        }
        if let cornerRadius = props["cornerRadius"] as? Double {
            config.cornerRadius = cornerRadius
        }
        if let qualities = props["qualities"] as? [[String: Any]] {
            config.qualities = qualities.compactMap { entry in
                guard let label = entry["label"] as? String else { return nil }
                let url = (entry["url"] as? String).flatMap(URL.init(string:))
                return LxMediaQuality(label: label, url: url)
            }
        }
        if let playbackRates = props["playbackRates"] as? [Any] {
            config.speeds = playbackRates.compactMap { ($0 as? NSNumber)?.doubleValue ?? ($0 as? Double) }
        }
        if let showControlsOnInit = props["showControlsOnInit"] as? Bool {
            config.showControlsOnInit = showControlsOnInit
        }
        if let objectFitRaw = props["objectFit"] as? String,
           let fit = LxMediaObjectFit(rawValue: objectFitRaw) {
            config.objectFit = fit
        }

        return config
    }

    private static func makeCommand(name: String, params: [String: Any]?) -> LxMediaCommand? {
        guard let command = CommandName(rawValue: name) else {
            return nil
        }

        switch command {
        case .play: return .play
        case .pause: return .pause
        case .stop: return .stop
        case .seek:
            if let time = params?["time"] as? Double { return .seek(time: time) }
            return nil
        case .setVolume:
            if let volume = params?["volume"] as? Double { return .setVolume(volume) }
            return nil
        case .setMuted:
            if let muted = params?["muted"] as? Bool { return .setMuted(muted) }
            return nil
        case .setPlaybackRate:
            if let rate = params?["rate"] as? Double { return .setPlaybackRate(rate) }
            return nil
        case .enterFullscreen: return .enterFullscreen
        case .exitFullscreen: return .exitFullscreen
        }
    }
}

#endif
