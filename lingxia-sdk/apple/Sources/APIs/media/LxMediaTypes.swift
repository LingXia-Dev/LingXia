import Foundation
import AVFoundation

// MARK: - Cross-platform media types shared between iOS and macOS players

enum LxMediaSource {
    case url(URL)
    case file(path: String)

    var bridgeValue: [String: Any] {
        switch self {
        case .url(let url):
            return ["type": "url", "value": url.absoluteString]
        case .file(let path):
            return ["type": "file", "value": path]
        }
    }
}

struct LxMediaQuality {
    var label: String
    var url: URL?

    init(label: String, url: URL?) {
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

enum LxMediaObjectFit: String {
    case cover
    case contain
    case fill
    case fit

    var bridgeValue: String {
        rawValue
    }
}

struct LxMediaPlayerConfig {
    var source: LxMediaSource?
    var src: URL?
    var poster: URL?
    var duration: Double?
    var autoplay: Bool?
    var loop: Bool?
    var muted: Bool?
    var volume: Double?
    var controls: Bool?
    var progressBar: Bool?
    var live: Bool?
    var cornerRadius: Double?
    var qualities: [LxMediaQuality]?
    var speeds: [Double]?
    var showControlsOnInit: Bool?
    var objectFit: LxMediaObjectFit?
    var rotateDegrees: Int?
    var clearProps: Set<String>?

    init(
        source: LxMediaSource? = nil,
        src: URL? = nil,
        poster: URL? = nil,
        duration: Double? = nil,
        autoplay: Bool? = nil,
        loop: Bool? = nil,
        muted: Bool? = nil,
        volume: Double? = nil,
        controls: Bool? = nil,
        progressBar: Bool? = nil,
        live: Bool? = nil,
        cornerRadius: Double? = nil,
        qualities: [LxMediaQuality]? = nil,
        speeds: [Double]? = nil,
        showControlsOnInit: Bool? = nil,
        objectFit: LxMediaObjectFit? = nil,
        rotateDegrees: Int? = nil,
        clearProps: Set<String>? = nil
    ) {
        self.source = source
        self.src = src
        self.poster = poster
        self.duration = duration
        self.autoplay = autoplay
        self.loop = loop
        self.muted = muted
        self.volume = volume
        self.controls = controls
        self.progressBar = progressBar
        self.live = live
        self.cornerRadius = cornerRadius
        self.qualities = qualities
        self.speeds = speeds
        self.showControlsOnInit = showControlsOnInit
        self.objectFit = objectFit
        self.rotateDegrees = rotateDegrees
        self.clearProps = clearProps
    }

    var bridgeValue: [String: Any] {
        var dict: [String: Any] = [:]
        if let source {
            dict["source"] = source.bridgeValue
        }
        if let src { dict["src"] = src.absoluteString }
        if let poster { dict["poster"] = poster.absoluteString }
        if let duration { dict["duration"] = duration }
        if let autoplay { dict["autoplay"] = autoplay }
        if let loop { dict["loop"] = loop }
        if let muted { dict["muted"] = muted }
        if let volume { dict["volume"] = volume }
        if let controls { dict["controls"] = controls }
        if let progressBar { dict["progressBar"] = progressBar }
        if let live { dict["live"] = live }
        if let cornerRadius { dict["cornerRadius"] = cornerRadius }
        if let qualities { dict["qualities"] = qualities.map { $0.bridgeValue } }
        if let speeds { dict["speeds"] = speeds }
        if let showControlsOnInit { dict["showControlsOnInit"] = showControlsOnInit }
        if let objectFit { dict["objectFit"] = objectFit.bridgeValue }
        if let rotateDegrees { dict["rotate"] = rotateDegrees }
        if let clearProps, !clearProps.isEmpty {
            dict["__clearProps"] = Array(clearProps)
        }
        return dict
    }
}

enum LxMediaCommand {
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

enum LxMediaEvent {
    case play
    case playing
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
        case .playing: return "playing"
        case .pause: return "pause"
        case .stop: return "stop"
        case .ended: return "ended"
        case .waiting: return "waiting"
        case .seeked: return "seeked"
        case .timeUpdate: return "timeupdate"
        case .rateChange: return "ratechange"
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
        case .play, .playing, .pause, .stop, .ended, .waiting:
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
