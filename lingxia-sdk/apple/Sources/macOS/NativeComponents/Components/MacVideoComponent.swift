#if os(macOS)
import Foundation
import AppKit
import CLingXiaRustAPI

@MainActor
final class MacVideoComponentFactory: MacNativeComponentFactory {
    func make(
        id: String,
        initialProps: [String: Any],
        eventSink: @escaping ([String: Any]) -> Void
    ) -> MacNativeComponent {
        MacVideoComponent(id: id, initialProps: initialProps, eventSink: eventSink)
    }
}

@MainActor
final class MacVideoComponent: NSObject, MacNativeComponent {
    let id: String
    let view: NSView

    private let player: MacLxMediaPlayer

    init(id: String, initialProps: [String: Any], eventSink: @escaping ([String: Any]) -> Void) {
        self.id = id
        self.player = MacLxMediaPlayer(eventSink: eventSink)
        self.view = player.view
        super.init()
        player.update(config: Self.makeConfig(from: initialProps))
    }

    func mount(in host: NSView) {
        host.addSubview(view)
    }

    func update(props: [String: Any]) {
        var config = Self.makeConfig(from: props)

        let nextVolume = Self.double(from: props["volume"])
        if let nextVolume {
            if abs(nextVolume - player.currentVolume()) < 0.000_1 {
                config.volume = nil
            }
        } else {
            config.volume = nil
        }

        let nextMuted = Self.bool(from: props["muted"])
        if nextMuted == nil || nextMuted == player.isMuted() {
            config.muted = nil
        }

        player.update(config: config)
    }

    func setFrame(_ frame: CGRect) {
        view.frame = frame
        player.layoutSubviews()
    }

    func focus() {
        view.isHidden = false
    }
    func blur() {
        player.handle(command: .pause)
    }

    func handleCommand(name: String, params: [String: Any]?) {
        if name == "setDuration" {
            let duration = Self.double(from: params?["duration"])
            player.setExternalDurationSeconds(duration)
            return
        }
        guard let command = Self.makeCommand(name: name, params: params) else { return }
        player.handle(command: command)
    }

    func unmount() {
        player.detach()
    }

    // MARK: - Helpers

    private static func url(from string: String) -> URL? {
        let raw = string.trimmingCharacters(in: .whitespacesAndNewlines)
        if raw.isEmpty { return nil }

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
        if let value = value as? String {
            let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
            if let parsed = Double(trimmed) { return parsed }
        }
        return nil
    }

    private static func bool(from value: Any?) -> Bool? {
        if let value = value as? Bool { return value }
        if let value = value as? NSNumber { return value.boolValue }
        return nil
    }

    private static func clearProps(from value: Any?) -> Set<String> {
        guard let array = value as? [Any] else { return [] }
        return Set(array.compactMap { item in
            if let key = item as? String {
                return key
            }
            if let key = item as? NSString {
                return key as String
            }
            return nil
        })
    }

    private static func parseSeekSeconds(from value: Any?, depth: Int = 0) -> Double? {
        if depth > 3 { return nil }

        if let seconds = double(from: value), seconds.isFinite, seconds >= 0 {
            return seconds
        }

        guard let object = value as? [String: Any] else { return nil }
        var zeroCandidate: Double?
        for key in ["time", "position", "currentTime", "value"] {
            if let seconds = parseSeekSeconds(from: object[key], depth: depth + 1) {
                if seconds > 0 {
                    return seconds
                }
                if seconds == 0 {
                    zeroCandidate = 0
                }
            }
        }
        return zeroCandidate
    }

    private enum CommandName: String {
        case play, pause, stop, seek
        case setVolume, setMuted, setPlaybackRate
        case enterFullscreen, exitFullscreen
    }

    private static func makeConfig(from props: [String: Any]) -> LxMediaPlayerConfig {
        var config = LxMediaPlayerConfig()
        let clearProps = Self.clearProps(from: props["__clearProps"])
        config.clearProps = clearProps

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

        if let duration = Self.double(from: props["duration"]), duration > 0 {
            config.duration = duration
        } else if let durationMs = Self.double(from: props["durationMs"]), durationMs > 0 {
            config.duration = durationMs / 1000.0
        }

        if let autoplay = props["autoplay"] as? Bool { config.autoplay = autoplay }
        if let loop = props["loop"] as? Bool { config.loop = loop }
        if let muted = Self.bool(from: props["muted"]) { config.muted = muted }
        if let volume = Self.double(from: props["volume"]) { config.volume = volume }
        if let controls = props["controls"] as? Bool { config.controls = controls }
        if let progressBar = props["progressBar"] as? Bool { config.progressBar = progressBar }
        if let cornerRadius = props["cornerRadius"] as? Double { config.cornerRadius = cornerRadius }
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
        if !clearProps.contains("objectFit"),
           let objectFitRaw = props["objectFit"] as? String,
           let fit = LxMediaObjectFit(rawValue: objectFitRaw) {
            config.objectFit = fit
        }
        if !clearProps.contains("rotate"),
           let rotation = Self.double(from: props["rotate"]) {
            config.rotateDegrees = Int(rotation)
        }

        return config
    }

    private static func makeCommand(name: String, params: [String: Any]?) -> LxMediaCommand? {
        guard let command = CommandName(rawValue: name) else { return nil }
        switch command {
        case .play: return .play
        case .pause: return .pause
        case .stop: return .stop
        case .seek:
            if let time = Self.parseSeekSeconds(from: params) { return .seek(time: time) }
            return nil
        case .setVolume:
            if let volume = Self.double(from: params?["volume"]), volume.isFinite { return .setVolume(volume) }
            return nil
        case .setMuted:
            if let muted = Self.bool(from: params?["muted"]) { return .setMuted(muted) }
            return nil
        case .setPlaybackRate:
            if let rate = Self.double(from: params?["rate"]), rate.isFinite, rate > 0 { return .setPlaybackRate(rate) }
            return nil
        case .enterFullscreen: return .enterFullscreen
        case .exitFullscreen: return .exitFullscreen
        }
    }
}

#endif
