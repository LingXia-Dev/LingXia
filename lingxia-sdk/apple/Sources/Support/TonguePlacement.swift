import CoreGraphics

/// Where a collapsed side panel's tongue sits along the panel's edge of a phone window.
enum TonguePlacement: Equatable {
    /// Flush with the physical edge, its top `offset` along the edge.
    case flush(offset: CGFloat)
    /// No clear stretch of the edge is long enough: centred, inset to the safe-area edge.
    case inset

    /// The clear position nearest the middle of an edge `length` long. `blocked` are the
    /// stretches a housing or cutout covers, kept `margin` clear; `start` and `end` are what the
    /// bars and rounded corners reserve at each end; `size` is the tongue's hit height.
    static func place(
        length: CGFloat, blocked: [ClosedRange<CGFloat>], start: CGFloat, end: CGFloat,
        size: CGFloat, margin: CGFloat
    ) -> TonguePlacement {
        let middle = (length - size) / 2
        var clear: [(CGFloat, CGFloat)] = []
        var from = start
        for block in blocked.sorted(by: { $0.lowerBound < $1.lowerBound }) {
            let low = block.lowerBound - margin
            if low > from { clear.append((from, low)) }
            from = max(from, block.upperBound + margin)
        }
        clear.append((from, length - end))
        let best = clear
            .filter { $0.1 - $0.0 >= size }
            .map { min(max(middle, $0.0), $0.1 - size) }
            .min { abs($0 - middle) < abs($1 - middle) }
        return best.map { .flush(offset: $0) } ?? .inset
    }
}
