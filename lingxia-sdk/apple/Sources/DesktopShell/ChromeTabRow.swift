#if os(macOS)
import AppKit

/// Shared geometry for Chrome-style title tabs — the ONE tab visual for every
/// aside slot header (browser and lxapp alike), so the strips read identical.
enum ChromeTabMetrics {
    static let barHeight: CGFloat = 36
    static let tabHeight: CGFloat = 28
    static let iconSize: CGFloat = 14
    static let minTabWidth: CGFloat = 120
    static let maxTabWidth: CGFloat = 220
    static let edge: CGFloat = 8
    /// Chrome-tab silhouette geometry, proportioned for the 28pt tab body.
    static let tabTopInset: CGFloat = 3
    static let tabFoot: CGFloat = 7
    static let tabCorner: CGFloat = 6
}

/// A Chrome-tab silhouette row: rounded top corners, bottom feet flaring into
/// the bar baseline, hover wash, and a trailing hairline divider between idle
/// neighbours. Content (icon/title/close) is arranged by the owner — this view
/// is an NSStackView and only owns the chrome drawing.
@MainActor
final class ChromeTabRowView: NSStackView {
    var isActiveTab = false {
        didSet {
            guard oldValue != isActiveTab else { return }
            needsDisplay = true
        }
    }

    private var isHovered = false {
        didSet {
            guard oldValue != isHovered else { return }
            needsDisplay = true
        }
    }

    private var tracking: NSTrackingArea?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
        layer?.masksToBounds = false
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var isFlipped: Bool { true }

    /// Suppresses this tab's trailing hairline divider when the seam is
    /// shared with the active tab (Chrome hides the divider by the selection).
    var suppressTrailingSeparator = false {
        didSet {
            guard oldValue != suppressTrailingSeparator else { return }
            needsDisplay = true
        }
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        let scale = max(1, window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2)
        let pixel = 1 / scale
        // Feet flare out to the full width at the baseline; the body sits
        // inset by `foot` on each side so neighbouring feet interlock.
        let topInset = ChromeTabMetrics.tabTopInset
        let foot = ChromeTabMetrics.tabFoot
        let corner = ChromeTabMetrics.tabCorner

        if isActiveTab {
            // The active tab is the page surfacing up through the bar: fill
            // with the content colour, flared feet merging into the toolbar
            // bottom (overdraw 1px to bridge the toolbar separator).
            let path = chromeTabPath(topInset: topInset, foot: foot, corner: corner, overdraw: pixel)
            NSColor.windowBackgroundColor.setFill()
            path.fill()
            // Whisper-thin top hairline for definition; the colour contrast
            // with the bar carries the separation (Chrome light has none).
            NSColor.separatorColor.withAlphaComponent(0.3).setStroke()
            path.lineWidth = pixel
            path.stroke()
        } else if isHovered {
            // Hover: a gently rounded wash inset from the edges, no feet.
            let washRect = NSRect(
                x: foot * 0.5,
                y: topInset + 1,
                width: max(0, bounds.width - foot),
                height: max(0, bounds.height - topInset - 1)
            )
            let path = NSBezierPath(
                roundedRect: washRect,
                xRadius: corner,
                yRadius: corner
            )
            NSColor.labelColor.withAlphaComponent(0.08).setFill()
            path.fill()
        } else if !suppressTrailingSeparator {
            // Idle tabs are transparent, divided by a short centred hairline.
            let inset: CGFloat = 8
            NSColor.separatorColor.withAlphaComponent(0.5).setFill()
            NSRect(
                x: bounds.width - pixel,
                y: topInset + inset,
                width: pixel,
                height: max(0, bounds.height - topInset - inset * 2)
            ).fill()
        }
    }

    /// A Chrome tab silhouette (flipped coords): rounded top corners and
    /// bottom feet that flare outward to the full width at the baseline.
    private func chromeTabPath(topInset: CGFloat, foot r: CGFloat, corner c: CGFloat, overdraw: CGFloat) -> NSBezierPath {
        let top = topInset
        let bottom = bounds.height + overdraw
        let left: CGFloat = 0
        let right = bounds.width
        let k: CGFloat = 0.5 // control-point pull for a soft quarter turn
        let path = NSBezierPath()
        // bottom-left foot (outer)
        path.move(to: NSPoint(x: left, y: bottom))
        // flare up into the left side
        path.curve(
            to: NSPoint(x: left + r, y: bottom - r),
            controlPoint1: NSPoint(x: left + r * k, y: bottom),
            controlPoint2: NSPoint(x: left + r, y: bottom - r * k)
        )
        path.line(to: NSPoint(x: left + r, y: top + c))
        // top-left corner
        path.curve(
            to: NSPoint(x: left + r + c, y: top),
            controlPoint1: NSPoint(x: left + r, y: top + c * k),
            controlPoint2: NSPoint(x: left + r + c * k, y: top)
        )
        path.line(to: NSPoint(x: right - r - c, y: top))
        // top-right corner
        path.curve(
            to: NSPoint(x: right - r, y: top + c),
            controlPoint1: NSPoint(x: right - r - c * k, y: top),
            controlPoint2: NSPoint(x: right - r, y: top + c * k)
        )
        path.line(to: NSPoint(x: right - r, y: bottom - r))
        // bottom-right foot flare
        path.curve(
            to: NSPoint(x: right, y: bottom),
            controlPoint1: NSPoint(x: right - r, y: bottom - r * k),
            controlPoint2: NSPoint(x: right - r * k, y: bottom)
        )
        path.close()
        return path
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking {
            removeTrackingArea(tracking)
        }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.activeInKeyWindow, .mouseEnteredAndExited, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
        tracking = area
    }

    override func mouseEntered(with event: NSEvent) {
        isHovered = true
    }

    override func mouseExited(with event: NSEvent) {
        isHovered = false
    }
}
#endif
