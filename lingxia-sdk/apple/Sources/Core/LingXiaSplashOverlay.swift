import Foundation
import OSLog

#if os(iOS)
import UIKit

private let splashLog = OSLog(subsystem: "LingXia", category: "Splash")

/// Runtime half of the launch screen: covers the screen from the home app's
/// cold start until its page first renders, drawing the same composition the
/// generated launch storyboard just handed over — the bundled art, filled, on
/// the brand ground — so there is nothing to see at the handoff. Dismissed by
/// the runtime's onHomeFirstReady signal, or handed to the host's campaign
/// screen first, with a timeout fallback so a broken page never leaves it
/// stuck.
///
/// It lives in its own window above the app's: the host swaps the app
/// window's `rootViewController` (and may present the lxapp manager modally)
/// right after startup, either of which would bury a plain subview overlay.
///
/// Assets are looked up by the names the CLI generates
/// (`LingXiaSplashBackground` color, `LingXiaSplash` cover,
/// `LingXiaSplashMark` mark); when the color is absent the overlay is
/// disabled entirely.
///
/// One face, drawn in every appearance: only one picture can be identical to
/// a frame the OS composed before this process existed.
@MainActor
enum LingXiaSplashOverlay {
    private static let timeoutSeconds: TimeInterval = 6
    /// The lift, and the campaign's fade-in. Both match Android and Harmony
    /// to the millisecond: one launch experience, three renderers.
    private static let liftSeconds: TimeInterval = 0.3
    private static let campaignFadeSeconds: TimeInterval = 0.2

    private static var splashWindow: UIWindow?
    private static var shownThisProcess = false
    private static var homeReadySeen = false
    private static var shownAt: CFAbsoluteTime = 0
    /// True from the campaign's first frame until the layer lifts — the
    /// countdown's own guard, and what tells the launch watchdog to stand down.
    private static var campaignActive = false

    /// Cover the screen at host startup, resolving the active scene's window.
    /// Idempotent — safe to call from every plausible cold-start entry point.
    static func attachIfNeeded() {
        guard !shownThisProcess, !homeReadySeen else { return }
        let window = UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .first?
            .windows
            .first
        guard let window else {
            os_log("splash skipped: no window yet", log: splashLog, type: .info)
            return
        }
        attachIfNeeded(to: window)
    }

    /// Cover the screen on the home app's cold start when splash assets exist.
    static func attachIfNeeded(to window: UIWindow) {
        guard !shownThisProcess, !homeReadySeen else { return }
        guard let scene = window.windowScene else {
            os_log("splash skipped: no window scene", log: splashLog, type: .info)
            return
        }
        // The appearance the launch frame is already showing. Everything below
        // resolves against this one value, so the art, the mark and the ground
        // can never come from different schemes — and the frame this replaces
        // came from the same one. The runtime is told which won, so a campaign
        // can match it.
        guard let background = resolveBackground() else {
            os_log("splash skipped: no background configured", log: splashLog, type: .info)
            return
        }
        LxAppCore.registerNativeHostAddonOnce()
        // The system appearance is reported to the runtime for the campaign
        // hook's sake; the face itself is one picture either way.
        splashMarkLaunchFace(window.traitCollection.userInterfaceStyle == .dark)

        let host = UIViewController()
        host.view.backgroundColor = background
        let coverImage = resolveCover()
        if let coverImage {
            // The launch face: full bleed, fully opaque from the very first
            // frame — the launch frame's handoff is the only transition onto
            // it. Any fade here would cross-blend the art with what sits
            // beneath, a beat the other platforms never show. Build-time art,
            // deliberately: nothing chosen at runtime can match a frame the OS
            // composed before this process existed.
            let cover = UIImageView(image: coverImage)
            cover.frame = host.view.bounds
            cover.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            cover.contentMode = .scaleAspectFill
            cover.clipsToBounds = true
            host.view.addSubview(cover)
        } else if let mark = resolveMark() {
            // No cover configured: reproduce the launch frame's own
            // composition — brand color plus the centered mark — so the only
            // transition the user ever sees is into the home page. The
            // catalog ships the mark as a 3x entry, so the frame draws it at
            // pixels/3 points; match that, not the screen scale.
            let size = CGSize(width: mark.size.width / 3, height: mark.size.height / 3)
            let view = UIImageView(image: mark)
            view.frame = CGRect(
                x: (host.view.bounds.width - size.width) / 2,
                y: (host.view.bounds.height - size.height) / 2,
                width: size.width,
                height: size.height
            )
            view.autoresizingMask = [
                .flexibleLeftMargin, .flexibleRightMargin,
                .flexibleTopMargin, .flexibleBottomMargin,
            ]
            host.view.addSubview(view)
        }

        // Paint the app's own window too. Anything that shows through before
        // or behind the cover — the host's first SwiftUI frame, the gap while
        // the lxapp manager is being presented — is then the brand color
        // instead of the system default white.
        window.backgroundColor = background

        let splash = UIWindow(windowScene: scene)
        splash.windowLevel = .normal + 100
        splash.rootViewController = host
        splash.isHidden = false

        splashWindow = splash
        shownThisProcess = true
        shownAt = CFAbsoluteTimeGetCurrent()
        os_log("splash shown (art: %{public}@)", log: splashLog, type: .info,
               coverImage == nil ? "none" : "bundled")

        DispatchQueue.main.asyncAfter(deadline: .now() + timeoutSeconds) {
            // A campaign is on screen with a countdown of its own; the
            // watchdog exists for a boot that never finished, not for this.
            guard !campaignActive else { return }
            if splashWindow != nil {
                os_log("splash dismissed by timeout", log: splashLog, type: .error)
            }
            dismiss()
        }
    }

    /// Runtime signal (via `LxApp.onHomeFirstReady`): home page rendered its first frame.
    static func notifyHomeReady() {
        homeReadySeen = true
        dismiss()
    }

    /// Runtime signal (via `LxApp.showSplashCampaign`): the home page is ready
    /// and the host has a screen of its own to show first. The launch layer
    /// stays up and takes the campaign's art with a skippable countdown, so
    /// there is never a gap between the two.
    static func showCampaign(path: String, durationMs: UInt32) {
        homeReadySeen = true
        guard let splash = splashWindow, let host = splash.rootViewController,
              let art = UIImage(contentsOfFile: path)
        else {
            // Unreadable art is not worth a stuck launch.
            dismiss()
            return
        }

        let view = UIImageView(image: art)
        view.frame = host.view.bounds
        view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.contentMode = .scaleAspectFill
        view.clipsToBounds = true
        view.alpha = 0
        host.view.addSubview(view)

        let skip = SplashSkipButton(seconds: Int(ceil(Double(durationMs) / 1000)))
        skip.alpha = 0
        skip.onTap = { dismiss() }
        host.view.addSubview(skip)
        skip.pin(toTopTrailingOf: host.view)

        // Fades in, unlike the launch face: this beat is content arriving, and
        // a cut here would read as the launch stuttering.
        UIView.animate(withDuration: campaignFadeSeconds) {
            view.alpha = 1
            skip.alpha = 1
        }
        campaignActive = true
        scheduleCampaignTick(skip)
        DispatchQueue.main.asyncAfter(
            deadline: .now() + TimeInterval(durationMs) / 1000
        ) {
            guard campaignActive else { return }
            dismiss()
        }
    }

    /// One second at a time rather than a repeating timer: the tick has to run
    /// on the main actor, and re-arming from inside it keeps the whole
    /// countdown there without handing a timer across isolation.
    private static func scheduleCampaignTick(_ skip: SplashSkipButton) {
        DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
            MainActor.assumeIsolated {
                guard campaignActive else { return }
                if skip.seconds > 1 {
                    skip.tick()
                    scheduleCampaignTick(skip)
                }
            }
        }
    }

    private static func dismiss() {
        campaignActive = false
        guard let splash = splashWindow else { return }
        splashWindow = nil
        os_log("splash dismissed after %{public}.2fs", log: splashLog, type: .info,
               CFAbsoluteTimeGetCurrent() - shownAt)
        // The face lifts away: a slight zoom under the fade reads as depth —
        // the home page is beneath it, not after it.
        UIView.animate(
            withDuration: liftSeconds,
            delay: 0,
            options: .curveEaseOut,
            animations: {
                splash.alpha = 0
                splash.rootViewController?.view.transform =
                    CGAffineTransform(scaleX: 1.06, y: 1.06)
            },
            completion: { _ in splash.isHidden = true }
        )
    }

    /// Plain bundle resources, deliberately not the catalog: `actool` is an
    /// external tool that can fail and leave the compiled catalog out of the
    /// app, and the overlay must not lose its images when it does.
    private static func bundleImage(_ name: String) -> UIImage? {
        guard let url = Bundle.main.url(forResource: name, withExtension: "png") else {
            return nil
        }
        return UIImage(contentsOfFile: url.path)
    }

    private static func resolveCover() -> UIImage? {
        bundleImage("LingXiaSplash")
    }

    private static func resolveMark() -> UIImage? {
        bundleImage("LingXiaSplashMark")
    }

    /// Background color from Info.plist (always written when `splash:` is
    /// configured), falling back to the asset-catalog color for a host whose
    /// plist predates it.
    private static func resolveBackground() -> UIColor? {
        if let hex = Bundle.main.object(forInfoDictionaryKey: "LingXiaSplashBackground") as? String,
           let color = UIColor(lingXiaHex: hex) {
            return color
        }
        return UIColor(named: "LingXiaSplashBackground")
    }
}

/// The campaign's countdown, and the only way past it. A skip control that is
/// hard to hit is the same as no skip control, so it is a full pill with a
/// generous tap target, clear of the status bar.
@MainActor
private final class SplashSkipButton: UIControl {
    private(set) var seconds: Int
    private let label = UILabel()
    var onTap: (() -> Void)?

    init(seconds: Int) {
        self.seconds = max(1, seconds)
        super.init(frame: .zero)
        backgroundColor = UIColor(white: 0, alpha: 0.4)
        layer.cornerRadius = 14
        label.textColor = .white
        label.font = .systemFont(ofSize: 13)
        label.textAlignment = .center
        label.translatesAutoresizingMaskIntoConstraints = false
        addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            label.topAnchor.constraint(equalTo: topAnchor, constant: 6),
            label.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -6),
        ])
        addTarget(self, action: #selector(tapped), for: .touchUpInside)
        render()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    func tick() {
        seconds -= 1
        render()
    }

    private func render() {
        let skip = L10n.string("lx_splash_skip")
        label.text = "\(skip) \(max(0, seconds))"
    }

    @objc private func tapped() {
        onTap?()
    }

    func pin(toTopTrailingOf parent: UIView) {
        translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            trailingAnchor.constraint(equalTo: parent.safeAreaLayoutGuide.trailingAnchor, constant: -16),
            topAnchor.constraint(equalTo: parent.safeAreaLayoutGuide.topAnchor, constant: 12),
        ])
    }
}

extension UIColor {
    /// Parse `#RRGGBB` as the CLI writes it — into Info.plist for the splash
    /// ground, and into `app.json` for the host theme the page canvas reads.
    convenience init?(lingXiaHex hex: String) {
        var value = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        if value.hasPrefix("#") { value.removeFirst() }
        guard value.count == 6, let rgb = UInt32(value, radix: 16) else { return nil }
        self.init(
            red: CGFloat((rgb >> 16) & 0xFF) / 255,
            green: CGFloat((rgb >> 8) & 0xFF) / 255,
            blue: CGFloat(rgb & 0xFF) / 255,
            alpha: 1
        )
    }
}

#else
import AppKit
import QuartzCore

private let splashLog = OSLog(subsystem: "LingXia", category: "Splash")

/// Launch cover for the macOS runner's phone frame. Desktop and pad shells
/// never call `armIfConfigured`, so both runtime signals stay no-ops there:
/// marking a face that has no cover would hold home-ready for `minDuration`
/// with nothing on screen.
///
/// The runner has no OS launch frame. The CLI passes the host `splash:`
/// through the environment; a bundled `LingXiaSplashBackground` is only the
/// fallback for a host that installed its own art.
@MainActor
enum LingXiaSplashOverlay {
    private static let timeoutSeconds: TimeInterval = 6
    private static let liftSeconds: TimeInterval = 0.3
    private static let campaignFadeSeconds: TimeInterval = 0.2
    private static let defaultMinimumSeconds: TimeInterval = 0.6

    private static var coverView: NSView?
    private static var armed = false
    private static var shownThisProcess = false
    private static var homeReadySeen = false
    private static var shownAt: CFAbsoluteTime = 0
    private static var campaignActive = false

    /// Remember that a cover will be shown, and tell the runtime which
    /// appearance it is. No view yet: the phone window does not exist until
    /// the home app opens.
    static func armIfConfigured() {
        guard !homeReadySeen, !shownThisProcess, !armed else { return }
        guard resolveBackground() != nil else { return }
        armed = true
        let dark = NSApp.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        splashMarkLaunchFace(dark)
    }

    /// Full-bleed cover over `host`. A home-ready that already fired must not
    /// flash a cover onto a page that is already on screen.
    static func attach(to host: NSView) {
        guard armed, !shownThisProcess, !homeReadySeen else { return }
        guard let background = resolveBackground() else { return }

        let cover = SplashCoverView()
        cover.wantsLayer = true
        cover.layer?.backgroundColor = background.cgColor
        pin(cover, to: host)

        if let image = resolveCover(), let cgImage = cgImage(of: image) {
            let art = NSView()
            art.wantsLayer = true
            art.layer?.contents = cgImage
            art.layer?.contentsGravity = .resizeAspectFill
            pin(art, to: cover)
        } else if let mark = resolveMark() {
            // The authored mark is 3x, matching the iOS catalog entry.
            let size = NSSize(width: mark.size.width / 3, height: mark.size.height / 3)
            let art = NSImageView()
            art.image = mark
            art.imageScaling = .scaleProportionallyUpOrDown
            art.translatesAutoresizingMaskIntoConstraints = false
            cover.addSubview(art)
            NSLayoutConstraint.activate([
                art.centerXAnchor.constraint(equalTo: cover.centerXAnchor),
                art.centerYAnchor.constraint(equalTo: cover.centerYAnchor),
                art.widthAnchor.constraint(equalToConstant: size.width),
                art.heightAnchor.constraint(equalToConstant: size.height),
            ])
        }

        coverView = cover
        shownThisProcess = true
        host.layoutSubtreeIfNeeded()
        shownAt = CFAbsoluteTimeGetCurrent()
        os_log("splash shown", log: splashLog, type: .info)

        Task { @MainActor in
            try? await Task.sleep(nanoseconds: UInt64(timeoutSeconds * 1_000_000_000))
            guard !homeReadySeen, !campaignActive, coverView != nil else { return }
            os_log("splash dismissed by timeout", log: splashLog, type: .error)
            dismiss()
        }
    }

    /// Status and nav bars are added after the cover, as later siblings.
    static func bringToFront() {
        guard let cover = coverView, let host = cover.superview else { return }
        host.addSubview(cover, positioned: .above, relativeTo: nil)
    }

    static func notifyHomeReady() {
        homeReadySeen = true
        afterMinimumDisplay { dismiss() }
    }

    static func showCampaign(path: String, durationMs: UInt32) {
        homeReadySeen = true
        afterMinimumDisplay { showCampaignNow(path: path, durationMs: durationMs) }
    }

    private static func afterMinimumDisplay(_ action: @escaping @MainActor () -> Void) {
        guard let cover = coverView else { return }
        let elapsed = CFAbsoluteTimeGetCurrent() - shownAt
        let remaining = max(0, minimumVisibleSeconds - elapsed)
        guard remaining > 0 else {
            action()
            return
        }
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: UInt64(remaining * 1_000_000_000))
            guard coverView === cover else { return }
            action()
        }
    }

    private static var minimumVisibleSeconds: TimeInterval {
        guard let raw = ProcessInfo.processInfo.environment["LINGXIA_RUNNER_SPLASH_MIN_DURATION_MS"],
              let milliseconds = Double(raw), milliseconds.isFinite else {
            return defaultMinimumSeconds
        }
        return min(max(milliseconds, 0), 6_000) / 1_000
    }

    private static func showCampaignNow(path: String, durationMs: UInt32) {
        guard let cover = coverView, let art = NSImage(contentsOfFile: path),
              let cgImage = cgImage(of: art)
        else {
            dismiss()
            return
        }

        let view = NSView()
        view.wantsLayer = true
        view.layer?.contents = cgImage
        view.layer?.contentsGravity = .resizeAspectFill
        view.alphaValue = 0
        pin(view, to: cover)

        let skip = SplashSkipControl(seconds: Int(ceil(Double(durationMs) / 1000)))
        skip.alphaValue = 0
        skip.onTap = { dismiss() }
        skip.pin(toTopTrailingOf: cover)
        fadeIn(view)
        fadeIn(skip)

        campaignActive = true
        scheduleCampaignTick(skip)
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: UInt64(durationMs) * 1_000_000)
            guard campaignActive else { return }
            dismiss()
        }
    }

    private static func scheduleCampaignTick(_ skip: SplashSkipControl) {
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: 1_000_000_000)
            guard campaignActive else { return }
            if skip.seconds > 1 {
                skip.tick()
                scheduleCampaignTick(skip)
            }
        }
    }

    private static func dismiss() {
        campaignActive = false
        guard let cover = coverView else { return }
        coverView = nil
        os_log(
            "splash dismissed after %{public}.2fs",
            log: splashLog,
            type: .info,
            CFAbsoluteTimeGetCurrent() - shownAt
        )
        lift(cover)
    }

    private static func lift(_ cover: NSView) {
        guard let layer = cover.layer, cover.bounds.width > 0 else {
            cover.removeFromSuperview()
            return
        }
        let anchor = CGPoint(x: 0.5, y: 0.5)
        let bounds = cover.bounds
        let newPoint = CGPoint(x: bounds.width * anchor.x, y: bounds.height * anchor.y)
        let oldPoint = CGPoint(
            x: bounds.width * layer.anchorPoint.x,
            y: bounds.height * layer.anchorPoint.y
        )
        var position = layer.position
        position.x += newPoint.x - oldPoint.x
        position.y += newPoint.y - oldPoint.y
        layer.anchorPoint = anchor
        layer.position = position

        let group = CAAnimationGroup()
        group.duration = liftSeconds
        group.timingFunction = CAMediaTimingFunction(name: .easeOut)
        group.fillMode = .forwards
        group.isRemovedOnCompletion = false
        let fade = CABasicAnimation(keyPath: "opacity")
        fade.fromValue = 1
        fade.toValue = 0
        let scale = CABasicAnimation(keyPath: "transform.scale")
        scale.fromValue = 1
        scale.toValue = 1.06
        group.animations = [fade, scale]
        layer.add(group, forKey: "lift")

        Task { @MainActor in
            try? await Task.sleep(nanoseconds: UInt64(liftSeconds * 1_000_000_000))
            cover.removeFromSuperview()
        }
    }

    private static func fadeIn(_ view: NSView) {
        view.wantsLayer = true
        let fade = CABasicAnimation(keyPath: "opacity")
        fade.fromValue = 0
        fade.toValue = 1
        fade.duration = campaignFadeSeconds
        fade.timingFunction = CAMediaTimingFunction(name: .easeOut)
        fade.fillMode = .forwards
        fade.isRemovedOnCompletion = false
        view.layer?.add(fade, forKey: "fade")
        view.alphaValue = 1
    }

    private static func pin(_ child: NSView, to parent: NSView) {
        child.translatesAutoresizingMaskIntoConstraints = false
        parent.addSubview(child)
        NSLayoutConstraint.activate([
            child.topAnchor.constraint(equalTo: parent.topAnchor),
            child.leadingAnchor.constraint(equalTo: parent.leadingAnchor),
            child.trailingAnchor.constraint(equalTo: parent.trailingAnchor),
            child.bottomAnchor.constraint(equalTo: parent.bottomAnchor),
        ])
    }

    private static func cgImage(of image: NSImage) -> CGImage? {
        var rect = CGRect(origin: .zero, size: image.size)
        return image.cgImage(forProposedRect: &rect, context: nil, hints: nil)
    }

    private static func bundleImage(_ name: String) -> NSImage? {
        guard let url = Bundle.main.url(forResource: name, withExtension: "png") else {
            return nil
        }
        return NSImage(contentsOf: url)
    }

    private static func resolveCover() -> NSImage? {
        if let path = envPath("LINGXIA_RUNNER_SPLASH_IMAGE"),
           let image = NSImage(contentsOfFile: path) {
            return image
        }
        return bundleImage("LingXiaSplash")
    }

    private static func resolveMark() -> NSImage? {
        if let path = envPath("LINGXIA_RUNNER_SPLASH_MARK"),
           let image = NSImage(contentsOfFile: path) {
            return image
        }
        return bundleImage("LingXiaSplashMark")
    }

    /// Environment wins: `lingxia dev` points at the host project, whose art
    /// is not copied into the generic Runner bundle.
    private static func resolveBackground() -> NSColor? {
        if let hex = ProcessInfo.processInfo.environment["LINGXIA_RUNNER_SPLASH_BACKGROUND"]?
            .trimmingCharacters(in: .whitespacesAndNewlines),
           !hex.isEmpty {
            return NSColor(lingXiaHex: hex)
        }
        if let hex = Bundle.main.object(forInfoDictionaryKey: "LingXiaSplashBackground") as? String,
           let color = NSColor(lingXiaHex: hex) {
            return color
        }
        return NSColor(named: "LingXiaSplashBackground")
    }

    private static func envPath(_ key: String) -> String? {
        let value = ProcessInfo.processInfo.environment[key]?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard let value, !value.isEmpty else { return nil }
        return value
    }
}

/// Opaque cover. An ordinary `NSView` does not swallow every pointing event.
@MainActor
private final class SplashCoverView: NSView {
    override func mouseDown(with event: NSEvent) {}
    override func rightMouseDown(with event: NSEvent) {}
    override func otherMouseDown(with event: NSEvent) {}
    override func scrollWheel(with event: NSEvent) {}
}

@MainActor
private final class SplashSkipControl: NSView {
    private(set) var seconds: Int
    private let label = NSTextField(labelWithString: "")
    var onTap: (() -> Void)?

    init(seconds: Int) {
        self.seconds = max(1, seconds)
        super.init(frame: .zero)
        wantsLayer = true
        layer?.backgroundColor = NSColor(white: 0, alpha: 0.4).cgColor
        layer?.cornerRadius = 14
        label.textColor = .white
        label.font = .systemFont(ofSize: 13)
        label.alignment = .center
        label.translatesAutoresizingMaskIntoConstraints = false
        addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            label.topAnchor.constraint(equalTo: topAnchor, constant: 6),
            label.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -6),
        ])
        render()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { nil }

    func tick() {
        seconds -= 1
        render()
    }

    private func render() {
        label.stringValue = "\(L10n.string("lx_splash_skip")) \(max(0, seconds))"
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        bounds.contains(convert(point, from: superview)) ? self : nil
    }

    override func mouseDown(with event: NSEvent) {
        onTap?()
    }

    func pin(toTopTrailingOf parent: NSView) {
        translatesAutoresizingMaskIntoConstraints = false
        parent.addSubview(self)
        NSLayoutConstraint.activate([
            trailingAnchor.constraint(equalTo: parent.trailingAnchor, constant: -16),
            topAnchor.constraint(equalTo: parent.topAnchor, constant: 12),
        ])
    }
}

extension NSColor {
    convenience init?(lingXiaHex hex: String) {
        var value = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        if value.hasPrefix("#") { value.removeFirst() }
        guard value.count == 6, let rgb = UInt32(value, radix: 16) else { return nil }
        self.init(
            srgbRed: CGFloat((rgb >> 16) & 0xFF) / 255,
            green: CGFloat((rgb >> 8) & 0xFF) / 255,
            blue: CGFloat(rgb & 0xFF) / 255,
            alpha: 1
        )
    }
}

#endif
