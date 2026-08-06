#if os(macOS)
import AppKit
import CoreGraphics

// Procedurally drawn cell glyphs.
//
// Box drawing, block elements and powerline separators are geometry, not
// typography: taken from a font they inherit its stem weights and side
// bearings, so borders in TUIs show hairline gaps at cell seams and shift
// when the font changes. Drawing them to the cell box instead makes every
// border meet exactly, at any font and any size — the same reason Ghostty
// and kitty ship a sprite font.
enum LingXiaTerminalSprite {
    /// Stem weight of one arm of a box-drawing glyph.
    private enum Weight {
        case none, light, heavy, double
    }

    /// The four arms of a box-drawing cell, from the center outwards.
    private struct Arms {
        var up: Weight = .none
        var right: Weight = .none
        var down: Weight = .none
        var left: Weight = .none
    }

    /// True when this codepoint is drawn rather than shaped.
    static func handles(_ scalar: UInt32) -> Bool {
        switch scalar {
        case 0x2500...0x259F: return true // box drawing + block elements
        case 0xE0B0...0xE0B3: return true // powerline separators
        default: return false
        }
    }

    /// Draw the sprite into `context`, whose coordinate space is the cell box
    /// `(0, 0, cell.width, cell.height)` in points, origin bottom-left.
    /// Returns false when the codepoint has no sprite.
    static func draw(scalar: UInt32, in context: CGContext, cell: CGSize, scale: CGFloat) -> Bool {
        // Stems land on whole device pixels: a half-pixel stem is exactly the
        // blur this sprite font exists to avoid.
        let light = max(1 / scale, ((cell.height / 12) * scale).rounded() / scale)
        context.setFillColor(CGColor(gray: 1, alpha: 1))
        context.setStrokeColor(CGColor(gray: 1, alpha: 1))

        switch scalar {
        case 0x2500...0x257F:
            guard let arms = boxArms(scalar) else {
                return drawSpecialBox(scalar, in: context, cell: cell, light: light, scale: scale)
            }
            drawArms(arms, in: context, cell: cell, light: light, scale: scale)
            return true
        case 0x2580...0x259F:
            return drawBlock(scalar, in: context, cell: cell, scale: scale)
        case 0xE0B0...0xE0B3:
            return drawPowerline(scalar, in: context, cell: cell, light: light)
        default:
            return false
        }
    }

    // MARK: Box drawing

    /// Arm weights per codepoint. The Unicode block is systematic: each shape
    /// enumerates its heavy-arm combinations in a fixed order, so the families
    /// are expanded from tables rather than written out 128 times.
    private static func boxArms(_ scalar: UInt32) -> Arms? {
        switch scalar {
        // Straight lines.
        case 0x2500: return Arms(right: .light, left: .light)
        case 0x2501: return Arms(right: .heavy, left: .heavy)
        case 0x2502: return Arms(up: .light, down: .light)
        case 0x2503: return Arms(up: .heavy, down: .heavy)

        // Corners: (first arm, second arm) with heavy variants in order
        // light-light, first heavy, second heavy, both heavy.
        case 0x250C...0x250F: return corner(scalar - 0x250C, a: \.right, b: \.down)
        case 0x2510...0x2513: return corner(scalar - 0x2510, a: \.left, b: \.down)
        case 0x2514...0x2517: return corner(scalar - 0x2514, a: \.right, b: \.up)
        case 0x2518...0x251B: return corner(scalar - 0x2518, a: \.left, b: \.up)

        // Tees: the stem, then the two halves of the crossing bar.
        case 0x251C...0x2523: return tee(scalar - 0x251C, stem: \.right, first: \.up, second: \.down)
        case 0x2524...0x252B: return tee(scalar - 0x2524, stem: \.left, first: \.up, second: \.down)
        case 0x252C...0x2533: return tee(scalar - 0x252C, stem: \.down, first: \.left, second: \.right)
        case 0x2534...0x253B: return tee(scalar - 0x2534, stem: \.up, first: \.left, second: \.right)

        // Crosses.
        case 0x253C...0x254B: return cross(scalar - 0x253C)

        // Double lines.
        case 0x2550: return Arms(right: .double, left: .double)
        case 0x2551: return Arms(up: .double, down: .double)
        case 0x2552: return Arms(right: .double, down: .light)
        case 0x2553: return Arms(right: .light, down: .double)
        case 0x2554: return Arms(right: .double, down: .double)
        case 0x2555: return Arms(down: .light, left: .double)
        case 0x2556: return Arms(down: .double, left: .light)
        case 0x2557: return Arms(down: .double, left: .double)
        case 0x2558: return Arms(up: .light, right: .double)
        case 0x2559: return Arms(up: .double, right: .light)
        case 0x255A: return Arms(up: .double, right: .double)
        case 0x255B: return Arms(up: .light, left: .double)
        case 0x255C: return Arms(up: .double, left: .light)
        case 0x255D: return Arms(up: .double, left: .double)
        case 0x255E: return Arms(up: .light, right: .double, down: .light)
        case 0x255F: return Arms(up: .double, right: .light, down: .double)
        case 0x2560: return Arms(up: .double, right: .double, down: .double)
        case 0x2561: return Arms(up: .light, down: .light, left: .double)
        case 0x2562: return Arms(up: .double, down: .double, left: .light)
        case 0x2563: return Arms(up: .double, down: .double, left: .double)
        case 0x2564: return Arms(right: .double, down: .light, left: .double)
        case 0x2565: return Arms(right: .light, down: .double, left: .light)
        case 0x2566: return Arms(right: .double, down: .double, left: .double)
        case 0x2567: return Arms(up: .light, right: .double, left: .double)
        case 0x2568: return Arms(up: .double, right: .light, left: .light)
        case 0x2569: return Arms(up: .double, right: .double, left: .double)
        case 0x256A: return Arms(up: .light, right: .double, down: .light, left: .double)
        case 0x256B: return Arms(up: .double, right: .light, down: .double, left: .light)
        case 0x256C: return Arms(up: .double, right: .double, down: .double, left: .double)

        // Half stems.
        case 0x2574: return Arms(left: .light)
        case 0x2575: return Arms(up: .light)
        case 0x2576: return Arms(right: .light)
        case 0x2577: return Arms(down: .light)
        case 0x2578: return Arms(left: .heavy)
        case 0x2579: return Arms(up: .heavy)
        case 0x257A: return Arms(right: .heavy)
        case 0x257B: return Arms(down: .heavy)

        // Mixed-weight straights.
        case 0x257C: return Arms(right: .heavy, left: .light)
        case 0x257D: return Arms(up: .light, down: .heavy)
        case 0x257E: return Arms(right: .light, left: .heavy)
        case 0x257F: return Arms(up: .heavy, down: .light)

        default: return nil // dashes, rounded corners and diagonals draw themselves
        }
    }

    private static func corner(
        _ variant: UInt32,
        a: WritableKeyPath<Arms, Weight>,
        b: WritableKeyPath<Arms, Weight>
    ) -> Arms {
        var arms = Arms()
        arms[keyPath: a] = variant == 1 || variant == 3 ? .heavy : .light
        arms[keyPath: b] = variant == 2 || variant == 3 ? .heavy : .light
        return arms
    }

    /// Heavy-arm sets for the eight variants of a tee, in Unicode order:
    /// none, stem, first, second, first+second, first+stem, second+stem, all.
    private static func tee(
        _ variant: UInt32,
        stem: WritableKeyPath<Arms, Weight>,
        first: WritableKeyPath<Arms, Weight>,
        second: WritableKeyPath<Arms, Weight>
    ) -> Arms {
        let heavy: [(Bool, Bool, Bool)] = [
            (false, false, false), (true, false, false), (false, true, false),
            (false, false, true), (false, true, true), (true, true, false),
            (true, false, true), (true, true, true),
        ]
        let (stemHeavy, firstHeavy, secondHeavy) = heavy[Int(min(variant, 7))]
        var arms = Arms()
        arms[keyPath: stem] = stemHeavy ? .heavy : .light
        arms[keyPath: first] = firstHeavy ? .heavy : .light
        arms[keyPath: second] = secondHeavy ? .heavy : .light
        return arms
    }

    /// Heavy-arm sets for the sixteen crosses, in Unicode order (U+253C…U+254B)
    /// as (left, right, up, down).
    private static func cross(_ variant: UInt32) -> Arms {
        let heavy: [(Bool, Bool, Bool, Bool)] = [
            (false, false, false, false), (true, false, false, false),
            (false, true, false, false), (true, true, false, false),
            (false, false, true, false), (false, false, false, true),
            (false, false, true, true), (true, false, true, false),
            (false, true, true, false), (true, false, false, true),
            (false, true, false, true), (true, true, true, false),
            (true, true, false, true), (true, false, true, true),
            (false, true, true, true), (true, true, true, true),
        ]
        let (left, right, up, down) = heavy[Int(min(variant, 15))]
        return Arms(
            up: up ? .heavy : .light,
            right: right ? .heavy : .light,
            down: down ? .heavy : .light,
            left: left ? .heavy : .light
        )
    }

    private static func drawArms(
        _ arms: Arms,
        in context: CGContext,
        cell: CGSize,
        light: CGFloat,
        scale: CGFloat
    ) {
        let centerX = snap(cell.width / 2, scale)
        let centerY = snap(cell.height / 2, scale)

        func stem(_ weight: Weight, horizontal: Bool, towardsEnd: Bool) {
            guard weight != .none else { return }
            let thickness = weight == .heavy ? light * 2 : light
            let gap = light * 2
            let offsets: [CGFloat] = weight == .double ? [-gap / 2, gap / 2] : [0]
            for offset in offsets {
                if horizontal {
                    let y = snap(centerY + offset - thickness / 2, scale)
                    let x = towardsEnd ? centerX : 0
                    let width = towardsEnd ? cell.width - centerX : centerX
                    // A double stem crosses the center, so it spans the full
                    // half plus the perpendicular pair's offset.
                    context.fill(CGRect(x: x, y: y, width: width, height: thickness))
                } else {
                    let x = snap(centerX + offset - thickness / 2, scale)
                    let y = towardsEnd ? centerY : 0
                    let height = towardsEnd ? cell.height - centerY : centerY
                    context.fill(CGRect(x: x, y: y, width: thickness, height: height))
                }
            }
        }

        stem(arms.left, horizontal: true, towardsEnd: false)
        stem(arms.right, horizontal: true, towardsEnd: true)
        stem(arms.down, horizontal: false, towardsEnd: false)
        stem(arms.up, horizontal: false, towardsEnd: true)
    }

    /// Dashes, rounded corners and diagonals: shapes the arm table cannot
    /// express.
    private static func drawSpecialBox(
        _ scalar: UInt32,
        in context: CGContext,
        cell: CGSize,
        light: CGFloat,
        scale: CGFloat
    ) -> Bool {
        let centerX = snap(cell.width / 2, scale)
        let centerY = snap(cell.height / 2, scale)
        switch scalar {
        // Dashed lines: (dash count, heavy, horizontal).
        case 0x2504, 0x2505, 0x2508, 0x2509, 0x254C, 0x254D:
            let count = scalar == 0x2504 || scalar == 0x2505 ? 3 : (scalar == 0x2508 || scalar == 0x2509 ? 4 : 2)
            let heavy = scalar % 2 == 1
            drawDashes(count: count, horizontal: true, heavy: heavy, in: context, cell: cell, light: light, scale: scale)
            return true
        case 0x2506, 0x2507, 0x250A, 0x250B, 0x254E, 0x254F:
            let count = scalar == 0x2506 || scalar == 0x2507 ? 3 : (scalar == 0x250A || scalar == 0x250B ? 4 : 2)
            let heavy = scalar % 2 == 1
            drawDashes(count: count, horizontal: false, heavy: heavy, in: context, cell: cell, light: light, scale: scale)
            return true

        // Rounded corners.
        case 0x256D...0x2570:
            let radius = min(cell.width, cell.height) / 2
            let path = CGMutablePath()
            switch scalar {
            case 0x256D: // ╭ right + down
                path.move(to: CGPoint(x: cell.width, y: centerY))
                path.addArc(tangent1End: CGPoint(x: centerX, y: centerY), tangent2End: CGPoint(x: centerX, y: 0), radius: radius)
                path.addLine(to: CGPoint(x: centerX, y: 0))
            case 0x256E: // ╮ left + down
                path.move(to: CGPoint(x: 0, y: centerY))
                path.addArc(tangent1End: CGPoint(x: centerX, y: centerY), tangent2End: CGPoint(x: centerX, y: 0), radius: radius)
                path.addLine(to: CGPoint(x: centerX, y: 0))
            case 0x256F: // ╯ left + up
                path.move(to: CGPoint(x: 0, y: centerY))
                path.addArc(tangent1End: CGPoint(x: centerX, y: centerY), tangent2End: CGPoint(x: centerX, y: cell.height), radius: radius)
                path.addLine(to: CGPoint(x: centerX, y: cell.height))
            default: // ╰ right + up
                path.move(to: CGPoint(x: cell.width, y: centerY))
                path.addArc(tangent1End: CGPoint(x: centerX, y: centerY), tangent2End: CGPoint(x: centerX, y: cell.height), radius: radius)
                path.addLine(to: CGPoint(x: centerX, y: cell.height))
            }
            context.setLineWidth(light)
            context.setLineCap(.butt)
            context.addPath(path)
            context.strokePath()
            return true

        // Diagonals.
        case 0x2571, 0x2572, 0x2573:
            context.setLineWidth(light)
            context.setLineCap(.round)
            if scalar != 0x2572 {
                context.move(to: CGPoint(x: 0, y: 0))
                context.addLine(to: CGPoint(x: cell.width, y: cell.height))
            }
            if scalar != 0x2571 {
                context.move(to: CGPoint(x: 0, y: cell.height))
                context.addLine(to: CGPoint(x: cell.width, y: 0))
            }
            context.strokePath()
            return true

        default:
            return false
        }
    }

    private static func drawDashes(
        count: Int,
        horizontal: Bool,
        heavy: Bool,
        in context: CGContext,
        cell: CGSize,
        light: CGFloat,
        scale: CGFloat
    ) {
        let thickness = heavy ? light * 2 : light
        let span = horizontal ? cell.width : cell.height
        // Equal dash and gap counts, with the gaps sized so the run starts and
        // ends with a dash.
        let unit = span / CGFloat(count * 2 - 1)
        for index in 0..<count {
            let start = CGFloat(index) * unit * 2
            let rect = horizontal
                ? CGRect(
                    x: snap(start, scale),
                    y: snap(cell.height / 2 - thickness / 2, scale),
                    width: snap(unit, scale),
                    height: thickness
                )
                : CGRect(
                    x: snap(cell.width / 2 - thickness / 2, scale),
                    y: snap(start, scale),
                    width: thickness,
                    height: snap(unit, scale)
                )
            context.fill(rect)
        }
    }

    // MARK: Block elements

    private static func drawBlock(
        _ scalar: UInt32,
        in context: CGContext,
        cell: CGSize,
        scale: CGFloat
    ) -> Bool {
        let width = cell.width
        let height = cell.height
        func fill(_ rect: CGRect) {
            context.fill(CGRect(
                x: snap(rect.minX, scale),
                y: snap(rect.minY, scale),
                width: snap(rect.width, scale),
                height: snap(rect.height, scale)
            ))
        }
        switch scalar {
        // Eighth blocks growing from an edge.
        case 0x2580: fill(CGRect(x: 0, y: height / 2, width: width, height: height / 2)) // upper half
        case 0x2581...0x2588: // lower eighths, one eighth at U+2581 through full at U+2588
            let eighths = CGFloat(scalar - 0x2580)
            fill(CGRect(x: 0, y: 0, width: width, height: height * eighths / 8))
        case 0x2589...0x258F: // left eighths, seven at U+2589 down to one at U+258F
            let eighths = CGFloat(0x2590 - scalar)
            fill(CGRect(x: 0, y: 0, width: width * eighths / 8, height: height))
        case 0x2590: fill(CGRect(x: width / 2, y: 0, width: width / 2, height: height)) // right half
        case 0x2591, 0x2592, 0x2593: // shades
            let coverage: CGFloat = scalar == 0x2591 ? 0.25 : (scalar == 0x2592 ? 0.5 : 0.75)
            context.setFillColor(CGColor(gray: 1, alpha: coverage))
            context.fill(CGRect(x: 0, y: 0, width: width, height: height))
            context.setFillColor(CGColor(gray: 1, alpha: 1))
        case 0x2594: fill(CGRect(x: 0, y: height * 7 / 8, width: width, height: height / 8)) // upper eighth
        case 0x2595: fill(CGRect(x: width * 7 / 8, y: 0, width: width / 8, height: height)) // right eighth
        case 0x2596...0x259F: // quadrants
            let quadrants = quadrantMask(scalar)
            if quadrants.contains(.upperLeft) { fill(CGRect(x: 0, y: height / 2, width: width / 2, height: height / 2)) }
            if quadrants.contains(.upperRight) { fill(CGRect(x: width / 2, y: height / 2, width: width / 2, height: height / 2)) }
            if quadrants.contains(.lowerLeft) { fill(CGRect(x: 0, y: 0, width: width / 2, height: height / 2)) }
            if quadrants.contains(.lowerRight) { fill(CGRect(x: width / 2, y: 0, width: width / 2, height: height / 2)) }
        default:
            return false
        }
        return true
    }

    private struct Quadrants: OptionSet {
        let rawValue: Int
        static let upperLeft = Quadrants(rawValue: 1 << 0)
        static let upperRight = Quadrants(rawValue: 1 << 1)
        static let lowerLeft = Quadrants(rawValue: 1 << 2)
        static let lowerRight = Quadrants(rawValue: 1 << 3)
    }

    private static func quadrantMask(_ scalar: UInt32) -> Quadrants {
        switch scalar {
        case 0x2596: return .lowerLeft
        case 0x2597: return .lowerRight
        case 0x2598: return .upperLeft
        case 0x2599: return [.upperLeft, .lowerLeft, .lowerRight]
        case 0x259A: return [.upperLeft, .lowerRight]
        case 0x259B: return [.upperLeft, .upperRight, .lowerLeft]
        case 0x259C: return [.upperLeft, .upperRight, .lowerRight]
        case 0x259D: return .upperRight
        case 0x259E: return [.upperRight, .lowerLeft]
        case 0x259F: return [.upperRight, .lowerLeft, .lowerRight]
        default: return []
        }
    }

    // MARK: Powerline

    private static func drawPowerline(
        _ scalar: UInt32,
        in context: CGContext,
        cell: CGSize,
        light: CGFloat
    ) -> Bool {
        let path = CGMutablePath()
        switch scalar {
        case 0xE0B0: // solid right triangle
            path.addLines(between: [
                CGPoint(x: 0, y: 0),
                CGPoint(x: cell.width, y: cell.height / 2),
                CGPoint(x: 0, y: cell.height),
            ])
            path.closeSubpath()
            context.addPath(path)
            context.fillPath()
        case 0xE0B1: // right chevron
            path.addLines(between: [
                CGPoint(x: 0, y: 0),
                CGPoint(x: cell.width, y: cell.height / 2),
                CGPoint(x: 0, y: cell.height),
            ])
            context.setLineWidth(light)
            context.setLineJoin(.miter)
            context.addPath(path)
            context.strokePath()
        case 0xE0B2: // solid left triangle
            path.addLines(between: [
                CGPoint(x: cell.width, y: 0),
                CGPoint(x: 0, y: cell.height / 2),
                CGPoint(x: cell.width, y: cell.height),
            ])
            path.closeSubpath()
            context.addPath(path)
            context.fillPath()
        default: // left chevron
            path.addLines(between: [
                CGPoint(x: cell.width, y: 0),
                CGPoint(x: 0, y: cell.height / 2),
                CGPoint(x: cell.width, y: cell.height),
            ])
            context.setLineWidth(light)
            context.setLineJoin(.miter)
            context.addPath(path)
            context.strokePath()
        }
        return true
    }

    private static func snap(_ value: CGFloat, _ scale: CGFloat) -> CGFloat {
        (value * scale).rounded() / scale
    }
}
#endif
