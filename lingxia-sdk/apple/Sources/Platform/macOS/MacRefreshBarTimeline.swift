#if os(macOS)
import Foundation

/// The run state behind the macOS refresh bar, as a plain value so it can be
/// checked without a window. Times are `CACurrentMediaTime()` seconds.
///
/// The bar always spans the full width. While a refresh runs, a translucent
/// base carries a soft band that sweeps left to right; with Reduce Motion the
/// solid bar breathes its opacity instead. Done turns it solid, then fades it.
/// The 0.6s minimum is the controller's: `complete` finishes at once.
struct MacRefreshBarTimeline {
    static let fadeIn: TimeInterval = 0.12
    /// One pass of the band across the bar.
    static let sweepPeriod: TimeInterval = 1.2
    /// Band width, as a share of the bar.
    static let bandShare = 0.35
    /// Each soft edge, as a share of the band.
    static let bandEdge = 0.3
    /// Opacity of the base under the band.
    static let baseAlpha = 0.4
    static let breathPeriod: TimeInterval = 1.6
    static let breathLow = 0.5
    static let solidDuration: TimeInterval = 0.15
    static let fadeOut: TimeInterval = 0.25

    enum Start: Equatable {
        /// Already running: nothing changes.
        case unchanged
        /// The previous run is still finishing: carry on from what is shown.
        case resumed
        /// Nothing on screen: fade in from nothing with a new sweep.
        case fresh
    }

    private(set) var since: TimeInterval?
    private(set) var completedAt: TimeInterval?
    private(set) var reduceMotion = false
    /// The band's clock. Kept across a resumed run, so the band never jumps.
    private(set) var sweepEpoch: TimeInterval = 0

    var isRunning: Bool { since != nil && completedAt == nil }
    /// When the finish under way has faded out.
    var finishEnd: TimeInterval? { completedAt.map { $0 + Self.solidDuration + Self.fadeOut } }

    func isShown(at now: TimeInterval) -> Bool {
        since != nil && (finishEnd.map { now < $0 } ?? true)
    }

    /// A new refresh.
    @discardableResult
    mutating func start(at now: TimeInterval, reduceMotion: Bool) -> Start {
        guard !isRunning else { return .unchanged }
        let resumed = isShown(at: now)
        if !resumed { sweepEpoch = now }
        since = now
        completedAt = nil
        self.reduceMotion = reduceMotion
        return resumed ? .resumed : .fresh
    }

    /// The refresh finished: turn solid, then fade out.
    @discardableResult
    mutating func complete(at now: TimeInterval) -> Bool {
        guard isRunning else { return false }
        completedAt = now
        return true
    }

    mutating func reset() {
        self = MacRefreshBarTimeline()
    }

    /// How far into a pass the band is at `now`.
    func sweepPhase(at now: TimeInterval) -> TimeInterval {
        max(now - sweepEpoch, 0).truncatingRemainder(dividingBy: Self.sweepPeriod)
    }
}
#endif
