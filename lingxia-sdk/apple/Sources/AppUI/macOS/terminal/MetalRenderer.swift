#if os(macOS)
import AppKit
import CoreText
import Metal
import QuartzCore
import simd

// GPU renderer for the terminal grid.
//
// The engine hands out a frame as fixed-size cells over one UTF-8 blob
// (`terminalSessionFrame`), so a frame reaches the GPU without JSON, without
// a String per cell, and without touching the rows that did not change.
// Everything the grid needs is a textured or solid quad: backgrounds,
// selection, glyphs from a CoreText-rasterized atlas, underlines, and the
// cursor. One instanced draw call covers the whole screen.

/// Mirrors `lingxia_terminal::FrameCell`. The layout is fixed by the engine:
/// three `UInt32` colors, a `UInt32` offset into the text blob, then four
/// `UInt8`s. Changing either side without the other corrupts every frame.
struct LingXiaTerminalFrameCell {
    var fg: UInt32 = 0
    var bg: UInt32 = 0
    var underlineColor: UInt32 = 0
    var textOffset: UInt32 = 0
    var textLen: UInt8 = 0
    var attrs: UInt8 = 0
    var underline: UInt8 = 0
    var columns: UInt8 = 0
}

enum LingXiaTerminalAttr {
    static let bold: UInt8 = 1 << 0
    static let italic: UInt8 = 1 << 1
    static let underline: UInt8 = 1 << 2
    static let strike: UInt8 = 1 << 3
    static let inverse: UInt8 = 1 << 4
    static let dim: UInt8 = 1 << 5
    static let hidden: UInt8 = 1 << 6
}

/// One frame copied out of the engine's retained buffers.
///
/// The engine's pointers stay valid only until the next frame call for the
/// session, so the two buffers are copied once here (two memcpys, no
/// per-cell work) and everything downstream reads Swift memory.
struct LingXiaTerminalGPUFrame {
    var cols = 0
    var rows = 0
    var generation: UInt64 = 0
    var imageGeneration: UInt64 = 0
    var cells: [LingXiaTerminalFrameCell] = []
    var text: [UInt8] = []
    var defaultForeground: UInt32 = 0xFFFF_FFFF
    var defaultBackground: UInt32 = 0x0000_00FF
    var cursorCol = 0
    var cursorRow = 0
    var cursorVisible = false
    var cursorStyle: UInt8 = 0
    var applicationCursor = false
    var bracketedPaste = false
    var alternateScreen = false
    var scrollbarTotal: UInt64 = 0
    var scrollbarOffset: UInt64 = 0
    var scrollbarLen: UInt64 = 0
    var exited = false

    func cell(row: Int, col: Int) -> LingXiaTerminalFrameCell? {
        guard row >= 0, col >= 0, row < rows, col < cols else { return nil }
        let index = row * cols + col
        return index < cells.count ? cells[index] : nil
    }

    /// Cluster bytes for a cell, as a slice of the frame's blob.
    func clusterBytes(_ cell: LingXiaTerminalFrameCell) -> ArraySlice<UInt8> {
        let start = Int(cell.textOffset)
        let end = start + Int(cell.textLen)
        guard cell.textLen > 0, end <= text.count else { return ArraySlice() }
        return text[start..<end]
    }

    func clusterString(_ cell: LingXiaTerminalFrameCell) -> String {
        let bytes = clusterBytes(cell)
        guard !bytes.isEmpty else { return "" }
        return String(decoding: bytes, as: UTF8.self)
    }

    /// Text of one row, used for selection and accessibility.
    func rowText(_ row: Int) -> String {
        guard row >= 0, row < rows else { return "" }
        var out = ""
        for col in 0..<cols {
            guard let cell = cell(row: row, col: col) else { continue }
            if cell.textLen == 0 {
                // A continuation column is covered by the cell before it;
                // a blank one is a real space.
                if cell.columns > 0 { out.append(" ") }
                continue
            }
            out += clusterString(cell)
        }
        return out
    }
}

/// Polls a session for frames. Returns nil when nothing changed, so a quiet
/// terminal costs one FFI call and no allocation.
enum LingXiaTerminalFrameSource {
    static func poll(sessionID: UInt64, since generation: UInt64) -> LingXiaTerminalGPUFrame? {
        // The engine writes these records; a layout mismatch would read
        // garbage rather than fail, so it is checked once per frame.
        assert(MemoryLayout<LingXiaTerminalFrameCell>.stride == 20, "FrameCell layout drifted from the engine")
        let handle = terminalSessionFrame(sessionID, generation)
        guard handle.changed else { return nil }
        guard handle.cells != 0, handle.cells_len > 0 else { return nil }

        var frame = LingXiaTerminalGPUFrame()
        frame.cols = Int(handle.cols)
        frame.rows = Int(handle.rows)
        frame.generation = handle.generation
        frame.imageGeneration = handle.image_generation
        let cellsPointer = UnsafeRawPointer(bitPattern: handle.cells)!
            .assumingMemoryBound(to: LingXiaTerminalFrameCell.self)
        frame.cells = Array(UnsafeBufferPointer(start: cellsPointer, count: Int(handle.cells_len)))
        if handle.text != 0, handle.text_len > 0 {
            let textPointer = UnsafeRawPointer(bitPattern: handle.text)!
                .assumingMemoryBound(to: UInt8.self)
            frame.text = Array(UnsafeBufferPointer(start: textPointer, count: Int(handle.text_len)))
        }
        frame.defaultForeground = handle.default_fg
        frame.defaultBackground = handle.default_bg
        frame.cursorCol = Int(handle.cursor_col)
        frame.cursorRow = Int(handle.cursor_row)
        frame.cursorVisible = handle.cursor_visible
        frame.cursorStyle = handle.cursor_style
        frame.applicationCursor = handle.application_cursor
        frame.bracketedPaste = handle.bracketed_paste
        frame.alternateScreen = handle.alternate_screen
        frame.scrollbarTotal = handle.scrollbar_total
        frame.scrollbarOffset = handle.scrollbar_offset
        frame.scrollbarLen = handle.scrollbar_len
        frame.exited = handle.exited
        return frame
    }

    /// `exited` without building a frame, for the poll loop's teardown check.
    static func hasExited(sessionID: UInt64, since generation: UInt64) -> Bool {
        terminalSessionFrame(sessionID, generation).exited
    }
}

// MARK: - Glyph atlas

/// One rasterized glyph in the atlas.
private struct LingXiaTerminalGlyphEntry {
    /// Atlas texture coordinates, normalized.
    let uv: SIMD4<Float>
    /// Quad size in points.
    let size: CGSize
    /// Offset from the pen point — the cell's left edge, on the baseline — to
    /// the quad's top-left corner, in render coordinates (y grows down).
    let bearing: CGPoint
    /// Color glyphs (emoji) keep their own pixels instead of being tinted.
    let isColor: Bool
}

/// A glyph the shaper produced, anchored to the cell its first character
/// occupied. A ligature covers several cells but anchors to the first, which
/// is how it stays on the grid.
private struct LingXiaTerminalShapedGlyph {
    let glyph: CGGlyph
    let font: CTFont
    let column: Int
}

/// CoreText shapes whole runs, not cells.
///
/// Shaping per cell can never produce a ligature: `!=` only becomes one glyph
/// if the font sees both characters together. Runs are shaped as text and the
/// resulting glyphs are mapped back to the cell each one started in, so
/// ligatures render while every glyph stays on the terminal grid.
private final class LingXiaTerminalShaper {
    private struct Key: Hashable {
        let text: String
        let style: UInt8
        let ligatures: Bool
    }

    private var cache: [Key: [LingXiaTerminalShapedGlyph]] = [:]
    private var fonts: [UInt8: NSFont] = [:]
    /// Same faces with contextual alternates disabled. Ligatures cannot be
    /// switched off through the `ligature` attribute — it only reaches `liga`,
    /// while programming ligatures are `calt`, which CoreText applies by
    /// default — so the off state needs its own font instances.
    private var plainFonts: [UInt8: NSFont] = [:]

    func reset(regular: NSFont, bold: NSFont, italic: NSFont, boldItalic: NSFont) {
        fonts = [0: regular, 1: bold, 2: italic, 3: boldItalic]
        plainFonts = fonts.mapValues(LingXiaTerminalFontVariant.withoutLigatures)
        cache.removeAll(keepingCapacity: true)
    }

    /// `columns[i]` is the terminal column of UTF-16 unit `i` of `text`.
    func shape(
        text: String,
        columns: [Int],
        style: UInt8,
        ligatures: Bool
    ) -> [LingXiaTerminalShapedGlyph] {
        let faces = ligatures ? fonts : plainFonts
        guard !text.isEmpty, let font = faces[style & 0x3] else { return [] }
        let key = Key(text: text, style: style & 0x3, ligatures: ligatures)
        if let cached = cache[key] {
            return remap(cached, columns: columns)
        }

        // Shape at column 0 so a cached run can be reused at any position.
        var zeroed = columns
        if let first = columns.first {
            zeroed = columns.map { $0 - first }
        }
        let attributes: [NSAttributedString.Key: Any] = [.font: font]
        let line = CTLineCreateWithAttributedString(
            NSAttributedString(string: text, attributes: attributes)
        )
        var shaped: [LingXiaTerminalShapedGlyph] = []
        for run in (CTLineGetGlyphRuns(line) as? [CTRun]) ?? [] {
            let count = CTRunGetGlyphCount(run)
            guard count > 0 else { continue }
            var glyphs = [CGGlyph](repeating: 0, count: count)
            var indices = [CFIndex](repeating: 0, count: count)
            CTRunGetGlyphs(run, CFRangeMake(0, count), &glyphs)
            CTRunGetStringIndices(run, CFRangeMake(0, count), &indices)
            let attributes = CTRunGetAttributes(run) as NSDictionary
            guard let runFont = attributes[kCTFontAttributeName as String] as? NSFont else { continue }
            for index in 0..<count {
                let offset = indices[index]
                let column = offset >= 0 && offset < zeroed.count ? zeroed[offset] : 0
                shaped.append(LingXiaTerminalShapedGlyph(
                    glyph: glyphs[index],
                    font: runFont as CTFont,
                    column: column
                ))
            }
        }
        if cache.count > 4096 {
            cache.removeAll(keepingCapacity: true)
        }
        cache[key] = shaped
        return remap(shaped, columns: columns)
    }

    /// Shift a run cached at column 0 to where it is being drawn.
    private func remap(
        _ shaped: [LingXiaTerminalShapedGlyph],
        columns: [Int]
    ) -> [LingXiaTerminalShapedGlyph] {
        guard let origin = columns.first, origin != 0 else { return shaped }
        return shaped.map {
            LingXiaTerminalShapedGlyph(glyph: $0.glyph, font: $0.font, column: $0.column + origin)
        }
    }
}

/// CoreText rasterizes each distinct glyph once; the GPU reuses it forever.
///
/// Shaping and rasterization stay on CoreText, which is what keeps CJK, Nerd
/// Font symbols and emoji correct — the GPU only composites the results.
private final class LingXiaTerminalGlyphAtlas {
    private struct GlyphKey: Hashable {
        let font: UInt
        let glyph: CGGlyph
    }

    private struct SpriteKey: Hashable {
        let scalar: UInt32
        let columns: UInt8
    }

    private let device: MTLDevice
    private let size = 2048
    private let padding: CGFloat = 2
    private(set) var texture: MTLTexture
    private var glyphs: [GlyphKey: LingXiaTerminalGlyphEntry?] = [:]
    private var sprites: [SpriteKey: LingXiaTerminalGlyphEntry] = [:]
    private var shelfX: CGFloat = 0
    private var shelfY: CGFloat = 0
    private var shelfHeight: CGFloat = 0
    private var full = false

    var font: NSFont
    var boldFont: NSFont
    var italicFont: NSFont
    var boldItalicFont: NSFont
    var cellSize: CGSize
    var baseline: CGFloat

    init?(device: MTLDevice, font: NSFont, cellSize: CGSize, baseline: CGFloat) {
        let descriptor = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .bgra8Unorm,
            width: size,
            height: size,
            mipmapped: false
        )
        descriptor.usage = [.shaderRead]
        descriptor.storageMode = .managed
        guard let texture = device.makeTexture(descriptor: descriptor) else { return nil }
        self.device = device
        self.texture = texture
        self.font = font
        self.boldFont = LingXiaTerminalFont.bold(size: font.pointSize)
        self.italicFont = LingXiaTerminalFont.italic(size: font.pointSize)
        self.boldItalicFont = LingXiaTerminalFont.boldItalic(size: font.pointSize)
        self.cellSize = cellSize
        self.baseline = baseline
    }

    /// Drop every cached glyph: the font, cell metrics or backing scale
    /// changed, so the rasterized pixels no longer match the grid.
    func reset(font: NSFont, cellSize: CGSize, baseline: CGFloat) {
        self.font = font
        self.boldFont = LingXiaTerminalFont.bold(size: font.pointSize)
        self.italicFont = LingXiaTerminalFont.italic(size: font.pointSize)
        self.boldItalicFont = LingXiaTerminalFont.boldItalic(size: font.pointSize)
        self.cellSize = cellSize
        self.baseline = baseline
        glyphs.removeAll(keepingCapacity: true)
        sprites.removeAll(keepingCapacity: true)
        shelfX = 0
        shelfY = 0
        shelfHeight = 0
        full = false
    }

    func styledFont(_ style: UInt8) -> NSFont {
        switch style & 0x3 {
        case 1: return boldFont
        case 2: return italicFont
        case 3: return boldItalicFont
        default: return font
        }
    }

    /// A shaped glyph, rasterized on first use.
    func entry(glyph: CGGlyph, font: CTFont, scale: CGFloat) -> LingXiaTerminalGlyphEntry? {
        let key = GlyphKey(font: CFHash(font), glyph: glyph)
        if let cached = glyphs[key] { return cached }
        let rasterized = rasterizeGlyph(glyph, font: font, scale: scale)
        // Cached even when nil: a blank glyph (space) must not be re-measured
        // on every frame.
        glyphs[key] = rasterized
        return rasterized
    }

    /// A procedurally drawn cell glyph (box drawing, blocks, powerline).
    func sprite(scalar: UInt32, columns: UInt8, scale: CGFloat) -> LingXiaTerminalGlyphEntry? {
        let key = SpriteKey(scalar: scalar, columns: columns)
        if let cached = sprites[key] { return cached }
        guard let rasterized = rasterizeSprite(scalar, columns: columns, scale: scale) else {
            return nil
        }
        sprites[key] = rasterized
        return rasterized
    }

    private func rasterizeGlyph(
        _ glyph: CGGlyph,
        font: CTFont,
        scale: CGFloat
    ) -> LingXiaTerminalGlyphEntry? {
        guard !full else { return nil }
        var subject = glyph
        var ink = CGRect.zero
        CTFontGetBoundingRectsForGlyphs(font, .default, &subject, &ink, 1)
        guard ink.width > 0, ink.height > 0, !ink.isNull, !ink.isInfinite else { return nil }

        // Nerd Font symbols and emoji routinely overflow their cell; scale
        // them to fit rather than clipping, so a status-line prompt renders
        // whole.
        var factor: CGFloat = 1
        let box = CGSize(width: cellSize.width * 2, height: cellSize.height)
        if ink.height > box.height || ink.width > box.width {
            factor = min(box.height / ink.height, box.width / ink.width)
        }

        let padPixels = (padding * scale).rounded()
        let padPoints = padPixels / scale
        let pixelWidth = Int((ink.width * factor * scale).rounded(.up) + padPixels * 2)
        let pixelHeight = Int((ink.height * factor * scale).rounded(.up) + padPixels * 2)
        guard pixelWidth > 0, pixelHeight > 0,
              let region = allocate(width: pixelWidth, height: pixelHeight) else {
            full = true
            return nil
        }

        let bytesPerRow = pixelWidth * 4
        var pixels = [UInt8](repeating: 0, count: bytesPerRow * pixelHeight)
        let drawn: Bool = pixels.withUnsafeMutableBytes { raw -> Bool in
            guard let context = makeContext(raw.baseAddress, pixelWidth, pixelHeight, bytesPerRow) else {
                return false
            }
            context.scaleBy(x: scale, y: scale)
            context.translateBy(x: padPoints, y: padPoints)
            context.scaleBy(x: factor, y: factor)
            // Grayscale antialiasing only: subpixel coverage cannot survive
            // being tinted by an arbitrary cell color in the shader.
            context.setAllowsAntialiasing(true)
            context.setShouldAntialias(true)
            context.setShouldSmoothFonts(false)
            context.setAllowsFontSubpixelPositioning(false)
            context.setShouldSubpixelPositionFonts(false)
            context.setShouldSubpixelQuantizeFonts(true)
            context.setFillColor(CGColor(gray: 1, alpha: 1))
            var position = CGPoint(x: -ink.minX, y: -ink.minY)
            CTFontDrawGlyphs(font, &subject, &position, 1, context)
            return true
        }
        guard drawn else { return nil }

        upload(pixels, region: region, width: pixelWidth, height: pixelHeight, bytesPerRow: bytesPerRow)
        return LingXiaTerminalGlyphEntry(
            uv: uv(region: region, width: pixelWidth, height: pixelHeight),
            size: CGSize(width: CGFloat(pixelWidth) / scale, height: CGFloat(pixelHeight) / scale),
            bearing: CGPoint(
                x: ink.minX * factor - padPoints,
                y: -(ink.maxY * factor + padPoints)
            ),
            isColor: CTFontGetSymbolicTraits(font).contains(.traitColorGlyphs)
        )
    }

    private func rasterizeSprite(
        _ scalar: UInt32,
        columns: UInt8,
        scale: CGFloat
    ) -> LingXiaTerminalGlyphEntry? {
        guard !full else { return nil }
        let cell = CGSize(width: cellSize.width * CGFloat(max(columns, 1)), height: cellSize.height)
        let pixelWidth = Int((cell.width * scale).rounded(.up))
        let pixelHeight = Int((cell.height * scale).rounded(.up))
        guard pixelWidth > 0, pixelHeight > 0,
              let region = allocate(width: pixelWidth, height: pixelHeight) else {
            full = true
            return nil
        }

        let bytesPerRow = pixelWidth * 4
        var pixels = [UInt8](repeating: 0, count: bytesPerRow * pixelHeight)
        let drawn: Bool = pixels.withUnsafeMutableBytes { raw -> Bool in
            guard let context = makeContext(raw.baseAddress, pixelWidth, pixelHeight, bytesPerRow) else {
                return false
            }
            context.scaleBy(x: scale, y: scale)
            return LingXiaTerminalSprite.draw(scalar: scalar, in: context, cell: cell, scale: scale)
        }
        guard drawn else { return nil }

        upload(pixels, region: region, width: pixelWidth, height: pixelHeight, bytesPerRow: bytesPerRow)
        return LingXiaTerminalGlyphEntry(
            uv: uv(region: region, width: pixelWidth, height: pixelHeight),
            size: cell,
            // Sprites fill the cell box, so they hang from the cell's top edge
            // rather than the baseline.
            bearing: CGPoint(x: 0, y: -baselineFromTop),
            isColor: false
        )
    }

    private var baselineFromTop: CGFloat { cellSize.height - baseline }

    private func makeContext(
        _ data: UnsafeMutableRawPointer?,
        _ width: Int,
        _ height: Int,
        _ bytesPerRow: Int
    ) -> CGContext? {
        CGContext(
            data: data,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: bytesPerRow,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue
                | CGBitmapInfo.byteOrder32Little.rawValue
        )
    }

    private func upload(
        _ pixels: [UInt8],
        region: (x: Int, y: Int),
        width: Int,
        height: Int,
        bytesPerRow: Int
    ) {
        texture.replace(
            region: MTLRegionMake2D(region.x, region.y, width, height),
            mipmapLevel: 0,
            withBytes: pixels,
            bytesPerRow: bytesPerRow
        )
    }

    private func uv(region: (x: Int, y: Int), width: Int, height: Int) -> SIMD4<Float> {
        let atlasSize = Float(size)
        return SIMD4<Float>(
            Float(region.x) / atlasSize,
            Float(region.y) / atlasSize,
            Float(region.x + width) / atlasSize,
            Float(region.y + height) / atlasSize
        )
    }

    /// Shelf packing: glyphs are all around one cell tall, so rows pack
    /// tightly and the allocator stays a few comparisons.
    private func allocate(width: Int, height: Int) -> (x: Int, y: Int)? {
        if shelfX + CGFloat(width) > CGFloat(size) {
            shelfX = 0
            shelfY += shelfHeight
            shelfHeight = 0
        }
        if shelfY + CGFloat(height) > CGFloat(size) { return nil }
        let origin = (x: Int(shelfX), y: Int(shelfY))
        shelfX += CGFloat(width)
        shelfHeight = max(shelfHeight, CGFloat(height))
        return origin
    }
}

private struct LingXiaTerminalQuad {
    var rect: SIMD4<Float>
    var color: SIMD4<Float>
    var uv: SIMD4<Float>
    /// x: 0 solid, 1 alpha-masked glyph, 2 color glyph.
    var mode: SIMD4<Float>
}

/// Geometry and colors the view supplies for one draw.
struct LingXiaTerminalRenderContext {
    var cellSize: CGSize
    var baseline: CGFloat
    var font: NSFont
    var scale: CGFloat
    var viewSize: CGSize
    var selection: [(row: Int, startCol: Int, endCol: Int)] = []
    var searchHighlights: [(row: Int, startCol: Int, endCol: Int, active: Bool)] = []
    var selectionColor: NSColor = .selectedTextBackgroundColor
    var selectionForegroundColor: NSColor = .selectedTextColor
    var cursorColor: NSColor = .white
    var drawCursor = true
    var dimOpacity: CGFloat = 0.58
    /// Shape runs with the font's ligatures (`!=`, `=>` in coding fonts).
    var ligatures = true
    var scrollbarColor: NSColor = NSColor.lxTerminalForeground.withAlphaComponent(0.25)
    /// IME pre-edit, drawn over the grid at the cursor with an underline.
    var markedText: String?
    var markedTextOrigin = LingXiaTerminalGridPoint(row: 0, col: 0)
    var markedTextColor: NSColor = .lxTerminalForeground
    var markedTextBackground: NSColor = NSColor.black.withAlphaComponent(0.85)
}

final class LingXiaTerminalMetalRenderer {
    private let device: MTLDevice
    private let queue: MTLCommandQueue
    private let pipeline: MTLRenderPipelineState
    private let sampler: MTLSamplerState
    private var atlas: LingXiaTerminalGlyphAtlas?
    private let shaper = LingXiaTerminalShaper()
    private var quads: [LingXiaTerminalQuad] = []
    private var instanceBuffer: MTLBuffer?
    private var atlasKey: String = ""

    /// One device for every pane: each split otherwise pays for its own
    /// device, queue and shader library.
    private static let sharedDevice = MTLCreateSystemDefaultDevice()

    init?() {
        guard let device = Self.sharedDevice,
              let queue = device.makeCommandQueue() else { return nil }
        let library: MTLLibrary
        do {
            library = try device.makeLibrary(source: Self.shaderSource, options: nil)
        } catch {
            NSLog("[LingXia] terminal metal shader failed: \(error)")
            return nil
        }
        let descriptor = MTLRenderPipelineDescriptor()
        descriptor.vertexFunction = library.makeFunction(name: "lx_terminal_vertex")
        descriptor.fragmentFunction = library.makeFunction(name: "lx_terminal_fragment")
        descriptor.colorAttachments[0].pixelFormat = .bgra8Unorm
        descriptor.colorAttachments[0].isBlendingEnabled = true
        descriptor.colorAttachments[0].rgbBlendOperation = .add
        descriptor.colorAttachments[0].alphaBlendOperation = .add
        // The fragment shader outputs premultiplied color, so the source
        // factor must be `.one`. With `.sourceAlpha` the coverage is applied
        // twice and every antialiased edge renders at alpha², which reads as
        // thin, washed-out text.
        descriptor.colorAttachments[0].sourceRGBBlendFactor = .one
        descriptor.colorAttachments[0].sourceAlphaBlendFactor = .one
        descriptor.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
        descriptor.colorAttachments[0].destinationAlphaBlendFactor = .oneMinusSourceAlpha
        guard let pipeline = try? device.makeRenderPipelineState(descriptor: descriptor) else {
            return nil
        }
        // Glyph quads are snapped so one texel covers exactly one pixel;
        // any interpolation at that point only softens the result.
        let samplerDescriptor = MTLSamplerDescriptor()
        samplerDescriptor.minFilter = .nearest
        samplerDescriptor.magFilter = .nearest
        guard let sampler = device.makeSamplerState(descriptor: samplerDescriptor) else {
            return nil
        }
        self.device = device
        self.queue = queue
        self.pipeline = pipeline
        self.sampler = sampler
    }

    func makeLayer() -> CAMetalLayer {
        let layer = CAMetalLayer()
        layer.device = device
        layer.pixelFormat = .bgra8Unorm
        layer.framebufferOnly = true
        layer.isOpaque = true
        return layer
    }

    func render(frame: LingXiaTerminalGPUFrame, context: LingXiaTerminalRenderContext, in layer: CAMetalLayer) {
        let (atlas, clear) = build(frame: frame, context: context)
        draw(layer: layer, clear: clear, atlas: atlas)
    }

    /// Render one frame into an image instead of onto the screen.
    ///
    /// Screenshot automation captures a window by replaying the view tree
    /// through CoreGraphics, which by construction cannot see a Metal layer —
    /// the terminal would come out blank. Rendering the same quads into an
    /// offscreen texture on demand keeps automation working without asking
    /// for the Screen Recording permission a window-server capture needs, and
    /// costs nothing on the normal path.
    func image(frame: LingXiaTerminalGPUFrame, context: LingXiaTerminalRenderContext) -> CGImage? {
        let width = Int((context.viewSize.width * context.scale).rounded())
        let height = Int((context.viewSize.height * context.scale).rounded())
        guard width > 0, height > 0 else { return nil }

        let descriptor = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .bgra8Unorm,
            width: width,
            height: height,
            mipmapped: false
        )
        descriptor.usage = [.renderTarget, .shaderRead]
        descriptor.storageMode = .managed
        guard let target = device.makeTexture(descriptor: descriptor) else { return nil }

        let (atlas, clear) = build(frame: frame, context: context)
        guard encode(into: target, clear: clear, atlas: atlas, viewSize: context.viewSize, synchronize: true) else {
            return nil
        }

        let bytesPerRow = width * 4
        var pixels = [UInt8](repeating: 0, count: bytesPerRow * height)
        target.getBytes(
            &pixels,
            bytesPerRow: bytesPerRow,
            from: MTLRegionMake2D(0, 0, width, height),
            mipmapLevel: 0
        )
        guard let provider = CGDataProvider(data: Data(pixels) as CFData) else { return nil }
        return CGImage(
            width: width,
            height: height,
            bitsPerComponent: 8,
            bitsPerPixel: 32,
            bytesPerRow: bytesPerRow,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGBitmapInfo(
                rawValue: CGImageAlphaInfo.premultipliedFirst.rawValue
                    | CGBitmapInfo.byteOrder32Little.rawValue
            ),
            provider: provider,
            decode: nil,
            shouldInterpolate: false,
            intent: .defaultIntent
        )
    }

    /// Build the frame's quads. Returns the atlas they reference and the
    /// clear color, which is the terminal's own background.
    private func build(
        frame: LingXiaTerminalGPUFrame,
        context: LingXiaTerminalRenderContext
    ) -> (LingXiaTerminalGlyphAtlas, SIMD4<Float>) {
        let atlas = ensureAtlas(context: context)
        quads.removeAll(keepingCapacity: true)

        let defaultBackground = Self.color(frame.defaultBackground, fallbackAlpha: 1)
        let defaultForeground = Self.color(frame.defaultForeground, fallbackAlpha: 1)
        appendBackgrounds(frame: frame, context: context, defaultForeground: defaultForeground)
        appendSearchHighlights(context: context)
        appendSelection(context: context)
        appendGlyphs(
            frame: frame,
            context: context,
            atlas: atlas,
            defaultForeground: defaultForeground,
            defaultBackground: defaultBackground
        )
        appendCursor(frame: frame, context: context, defaultBackground: defaultBackground)
        appendMarkedText(context: context, atlas: atlas)
        appendScrollbar(frame: frame, context: context)
        return (atlas, defaultBackground)
    }

    // MARK: Quad building

    private func appendBackgrounds(
        frame: LingXiaTerminalGPUFrame,
        context: LingXiaTerminalRenderContext,
        defaultForeground: SIMD4<Float>
    ) {
        for row in 0..<frame.rows {
            var col = 0
            while col < frame.cols {
                guard let cell = frame.cell(row: row, col: col) else { break }
                let span = max(Int(cell.columns), 1)
                let inverse = cell.attrs & LingXiaTerminalAttr.inverse != 0
                // Alpha 0 marks the engine's "default background" sentinel, so
                // an untouched cell is left to the clear color.
                let hasBackground = (cell.bg & 0xFF) != 0
                if inverse || hasBackground {
                    let color = inverse
                        ? (cell.fg & 0xFF) != 0 ? Self.color(cell.fg, fallbackAlpha: 1) : defaultForeground
                        : Self.color(cell.bg, fallbackAlpha: 1)
                    quads.append(solid(
                        rect: Self.cellRect(row: row, col: col, span: span, context: context),
                        color: color
                    ))
                }
                col += span
            }
        }
    }

    private func appendSelection(context: LingXiaTerminalRenderContext) {
        guard !context.selection.isEmpty else { return }
        let color = Self.color(context.selectionColor)
        for span in context.selection where span.endCol > span.startCol {
            let rect = CGRect(
                x: CGFloat(span.startCol) * context.cellSize.width,
                y: CGFloat(span.row) * context.cellSize.height,
                width: CGFloat(span.endCol - span.startCol) * context.cellSize.width,
                height: context.cellSize.height
            )
            quads.append(solid(rect: rect, color: color))
        }
    }

    private func appendSearchHighlights(context: LingXiaTerminalRenderContext) {
        for span in context.searchHighlights where span.endCol > span.startCol {
            let base = span.active ? NSColor.systemOrange : NSColor.systemYellow
            let color = Self.color(base.withAlphaComponent(span.active ? 0.52 : 0.25))
            let rect = CGRect(
                x: CGFloat(span.startCol) * context.cellSize.width,
                y: CGFloat(span.row) * context.cellSize.height,
                width: CGFloat(span.endCol - span.startCol) * context.cellSize.width,
                height: context.cellSize.height
            )
            quads.append(solid(rect: rect, color: color))
        }
    }

    private func appendGlyphs(
        frame: LingXiaTerminalGPUFrame,
        context: LingXiaTerminalRenderContext,
        atlas: LingXiaTerminalGlyphAtlas,
        defaultForeground: SIMD4<Float>,
        defaultBackground: SIMD4<Float>
    ) {
        // A run is a stretch of cells that shape and paint as one: same font
        // style and same colors. Breaking on color as well as style keeps a
        // ligature from spanning two differently colored cells.
        var runText = ""
        var runColumns: [Int] = []
        var runStyle: UInt8 = 0
        var runColor = SIMD4<Float>()
        var runBackground = SIMD4<Float>()
        var runOpen = false

        func flushRun(row: Int) {
            defer {
                runText.removeAll(keepingCapacity: true)
                runColumns.removeAll(keepingCapacity: true)
                runOpen = false
            }
            guard runOpen, !runText.isEmpty else { return }
            let exponent = Self.coverageExponent(text: runColor, background: runBackground)
            for shaped in shaper.shape(
                text: runText,
                columns: runColumns,
                style: runStyle,
                ligatures: context.ligatures
            ) {
                guard let entry = atlas.entry(
                    glyph: shaped.glyph,
                    font: shaped.font,
                    scale: context.scale
                ) else { continue }
                let cell = Self.cellRect(row: row, col: shaped.column, span: 1, context: context)
                append(
                    entry: entry,
                    pen: CGPoint(x: cell.minX, y: cell.minY + context.baselineFromTop),
                    color: runColor,
                    exponent: exponent,
                    scale: context.scale
                )
            }
        }

        for row in 0..<frame.rows {
            var col = 0
            while col < frame.cols {
                guard let cell = frame.cell(row: row, col: col) else { break }
                let span = max(Int(cell.columns), 1)
                defer { col += span }

                if cell.textLen == 0 || cell.attrs & LingXiaTerminalAttr.hidden != 0 {
                    flushRun(row: row)
                    continue
                }

                let inverse = cell.attrs & LingXiaTerminalAttr.inverse != 0
                var color = inverse
                    ? (cell.bg & 0xFF) != 0 ? Self.color(cell.bg, fallbackAlpha: 1) : defaultBackground
                    : (cell.fg & 0xFF) != 0 ? Self.color(cell.fg, fallbackAlpha: 1) : defaultForeground
                if cell.attrs & LingXiaTerminalAttr.dim != 0 {
                    color.w *= Float(context.dimOpacity)
                }
                var background = inverse
                    ? (cell.fg & 0xFF) != 0 ? Self.color(cell.fg, fallbackAlpha: 1) : defaultForeground
                    : (cell.bg & 0xFF) != 0 ? Self.color(cell.bg, fallbackAlpha: 1) : defaultBackground
                if Self.isSelected(row: row, col: col, span: span, context: context) {
                    color = Self.color(context.selectionForegroundColor)
                    background = Self.color(context.selectionColor)
                }

                // Box drawing and friends are geometry, not text: they leave
                // the run and are drawn from the sprite font.
                let cluster = frame.clusterString(cell)
                if cluster.unicodeScalars.count == 1,
                   let scalar = cluster.unicodeScalars.first?.value,
                   LingXiaTerminalSprite.handles(scalar) {
                    flushRun(row: row)
                    if let entry = atlas.sprite(
                        scalar: scalar,
                        columns: UInt8(min(span, 8)),
                        scale: context.scale
                    ) {
                        let rect = Self.cellRect(row: row, col: col, span: span, context: context)
                        append(
                            entry: entry,
                            pen: CGPoint(x: rect.minX, y: rect.minY + context.baselineFromTop),
                            color: color,
                            exponent: 1,
                            scale: context.scale
                        )
                    }
                    continue
                }

                var style: UInt8 = 0
                if cell.attrs & LingXiaTerminalAttr.bold != 0 { style |= 1 }
                if cell.attrs & LingXiaTerminalAttr.italic != 0 { style |= 2 }
                if runOpen, style != runStyle || color != runColor || background != runBackground {
                    flushRun(row: row)
                }
                if !runOpen {
                    runStyle = style
                    runColor = color
                    runBackground = background
                    runOpen = true
                }
                runText += cluster
                runColumns.append(contentsOf: repeatElement(col, count: cluster.utf16.count))
            }
            flushRun(row: row)
        }

        appendDecorations(
            frame: frame,
            context: context,
            defaultForeground: defaultForeground,
            defaultBackground: defaultBackground
        )
    }

    /// Underlines and strikethrough, which follow the cell grid rather than
    /// the shaped glyphs.
    private func appendDecorations(
        frame: LingXiaTerminalGPUFrame,
        context: LingXiaTerminalRenderContext,
        defaultForeground: SIMD4<Float>,
        defaultBackground: SIMD4<Float>
    ) {
        for row in 0..<frame.rows {
            var col = 0
            while col < frame.cols {
                guard let cell = frame.cell(row: row, col: col) else { break }
                let span = max(Int(cell.columns), 1)
                defer { col += span }
                let decorated = cell.underline != 0
                    || cell.attrs & LingXiaTerminalAttr.strike != 0
                guard decorated else { continue }

                let inverse = cell.attrs & LingXiaTerminalAttr.inverse != 0
                var color = inverse
                    ? (cell.bg & 0xFF) != 0 ? Self.color(cell.bg, fallbackAlpha: 1) : defaultBackground
                    : (cell.fg & 0xFF) != 0 ? Self.color(cell.fg, fallbackAlpha: 1) : defaultForeground
                if cell.attrs & LingXiaTerminalAttr.dim != 0 {
                    color.w *= Float(context.dimOpacity)
                }
                if Self.isSelected(row: row, col: col, span: span, context: context) {
                    color = Self.color(context.selectionForegroundColor)
                }
                let rect = Self.cellRect(row: row, col: col, span: span, context: context)
                if cell.underline != 0 {
                    let underlineColor = (cell.underlineColor & 0xFF) != 0
                        ? Self.color(cell.underlineColor, fallbackAlpha: 1)
                        : color
                    quads.append(contentsOf: underlineQuads(
                        style: cell.underline,
                        rect: rect,
                        context: context,
                        color: underlineColor
                    ))
                }
                if cell.attrs & LingXiaTerminalAttr.strike != 0 {
                    let thickness = max(1 / context.scale, (context.cellSize.height / 14).rounded())
                    quads.append(solid(
                        rect: CGRect(
                            x: rect.minX,
                            y: rect.minY + context.baselineFromTop * 0.62,
                            width: rect.width,
                            height: thickness
                        ),
                        color: color
                    ))
                }
            }
        }
    }

    private static func isSelected(
        row: Int,
        col: Int,
        span: Int,
        context: LingXiaTerminalRenderContext
    ) -> Bool {
        context.selection.contains {
            $0.row == row && col < $0.endCol && col + span > $0.startCol
        }
    }

    /// Place a rasterized glyph at a pen point, snapped to whole pixels so one
    /// texel covers one pixel.
    private func append(
        entry: LingXiaTerminalGlyphEntry,
        pen: CGPoint,
        color: SIMD4<Float>,
        exponent: Float,
        scale: CGFloat
    ) {
        let x = Self.snap(pen.x + entry.bearing.x, scale)
        let y = Self.snap(pen.y + entry.bearing.y, scale)
        quads.append(LingXiaTerminalQuad(
            rect: SIMD4<Float>(
                Float(x), Float(y),
                Float(entry.size.width), Float(entry.size.height)
            ),
            color: color,
            uv: entry.uv,
            mode: SIMD4<Float>(entry.isColor ? 2 : 1, exponent, 0, 0)
        ))
    }

    private func underlineQuads(
        style: UInt8,
        rect: CGRect,
        context: LingXiaTerminalRenderContext,
        color: SIMD4<Float>
    ) -> [LingXiaTerminalQuad] {
        let thickness = max(1 / context.scale, (context.cellSize.height / 14).rounded())
        let y = rect.minY + context.baselineFromTop + thickness
        switch style {
        case 2: // double
            return [
                solid(rect: CGRect(x: rect.minX, y: y, width: rect.width, height: thickness), color: color),
                solid(
                    rect: CGRect(x: rect.minX, y: y + thickness * 2, width: rect.width, height: thickness),
                    color: color
                ),
            ]
        case 3: // curly, approximated by a thicker line until the shader draws a wave
            return [solid(
                rect: CGRect(x: rect.minX, y: y, width: rect.width, height: thickness * 2),
                color: color
            )]
        case 4, 5: // dotted / dashed
            let step = style == 4 ? thickness * 2 : thickness * 4
            var dashes: [LingXiaTerminalQuad] = []
            var x = rect.minX
            while x < rect.maxX {
                let width = min(step, rect.maxX - x)
                dashes.append(solid(
                    rect: CGRect(x: x, y: y, width: width, height: thickness),
                    color: color
                ))
                x += step * 2
            }
            return dashes
        default:
            return [solid(
                rect: CGRect(x: rect.minX, y: y, width: rect.width, height: thickness),
                color: color
            )]
        }
    }

    private func appendCursor(
        frame: LingXiaTerminalGPUFrame,
        context: LingXiaTerminalRenderContext,
        defaultBackground: SIMD4<Float>
    ) {
        guard context.drawCursor, frame.cursorVisible else { return }
        let span = max(Int(frame.cell(row: frame.cursorRow, col: frame.cursorCol)?.columns ?? 1), 1)
        let rect = Self.cellRect(row: frame.cursorRow, col: frame.cursorCol, span: span, context: context)
        let color = Self.color(context.cursorColor)
        let thickness = max(1 / context.scale, (context.cellSize.height / 12).rounded())
        switch frame.cursorStyle {
        case 1: // bar
            quads.append(solid(
                rect: CGRect(x: rect.minX, y: rect.minY, width: thickness, height: rect.height),
                color: color
            ))
        case 2: // underline
            quads.append(solid(
                rect: CGRect(x: rect.minX, y: rect.maxY - thickness, width: rect.width, height: thickness),
                color: color
            ))
        case 3: // hollow block
            quads.append(solid(rect: CGRect(x: rect.minX, y: rect.minY, width: rect.width, height: thickness), color: color))
            quads.append(solid(rect: CGRect(x: rect.minX, y: rect.maxY - thickness, width: rect.width, height: thickness), color: color))
            quads.append(solid(rect: CGRect(x: rect.minX, y: rect.minY, width: thickness, height: rect.height), color: color))
            quads.append(solid(rect: CGRect(x: rect.maxX - thickness, y: rect.minY, width: thickness, height: rect.height), color: color))
        default: // block, with the glyph under it redrawn in the background color
            quads.append(solid(rect: rect, color: color))
            if let cell = frame.cell(row: frame.cursorRow, col: frame.cursorCol),
               cell.textLen > 0,
               let atlas {
                appendText(
                    frame.clusterString(cell),
                    at: rect,
                    color: defaultBackground,
                    context: context,
                    atlas: atlas
                )
            }
        }
    }

    /// Draw a short string into a cell box: the cursor's covered glyph and the
    /// IME pre-edit, neither of which belongs to the grid's runs.
    private func appendText(
        _ text: String,
        at rect: CGRect,
        color: SIMD4<Float>,
        context: LingXiaTerminalRenderContext,
        atlas: LingXiaTerminalGlyphAtlas
    ) {
        guard !text.isEmpty else { return }
        let columns = Array(repeating: 0, count: text.utf16.count)
        for shaped in shaper.shape(text: text, columns: columns, style: 0, ligatures: false) {
            guard let entry = atlas.entry(
                glyph: shaped.glyph,
                font: shaped.font,
                scale: context.scale
            ) else { continue }
            append(
                entry: entry,
                pen: CGPoint(x: rect.minX, y: rect.minY + context.baselineFromTop),
                color: color,
                exponent: 1,
                scale: context.scale
            )
        }
    }

    /// Pre-edit text sits on top of the grid: its own background so the
    /// characters underneath do not show through, then the composing
    /// underline the input method expects.
    private func appendMarkedText(
        context: LingXiaTerminalRenderContext,
        atlas: LingXiaTerminalGlyphAtlas
    ) {
        guard let marked = context.markedText, !marked.isEmpty else { return }
        let color = Self.color(context.markedTextColor)
        var col = context.markedTextOrigin.col
        let row = context.markedTextOrigin.row
        let thickness = max(1 / context.scale, (context.cellSize.height / 14).rounded())
        for character in marked {
            let cluster = String(character)
            // Wide characters (the common case for CJK input) take two cells.
            let columns: UInt8 = cluster.unicodeScalars.contains { $0.value >= 0x1100 } ? 2 : 1
            let rect = Self.cellRect(row: row, col: col, span: Int(columns), context: context)
            quads.append(solid(rect: rect, color: Self.color(context.markedTextBackground)))
            appendText(cluster, at: rect, color: color, context: context, atlas: atlas)
            quads.append(solid(
                rect: CGRect(
                    x: rect.minX,
                    y: rect.maxY - thickness,
                    width: rect.width,
                    height: thickness
                ),
                color: color
            ))
            col += Int(columns)
        }
    }

    private func appendScrollbar(frame: LingXiaTerminalGPUFrame, context: LingXiaTerminalRenderContext) {
        guard frame.scrollbarTotal > UInt64(frame.rows), frame.scrollbarLen > 0 else { return }
        let width: CGFloat = 4
        let total = CGFloat(frame.scrollbarTotal)
        let height = max(context.viewSize.height * CGFloat(frame.scrollbarLen) / total, 24)
        let travel = max(context.viewSize.height - height, 0)
        let progress = total > CGFloat(frame.scrollbarLen)
            ? CGFloat(frame.scrollbarOffset) / (total - CGFloat(frame.scrollbarLen))
            : 0
        quads.append(solid(
            rect: CGRect(
                x: context.viewSize.width - width - 2,
                y: travel * min(max(progress, 0), 1),
                width: width,
                height: height
            ),
            color: Self.color(context.scrollbarColor)
        ))
    }

    // MARK: Drawing

    private func draw(layer: CAMetalLayer, clear: SIMD4<Float>, atlas: LingXiaTerminalGlyphAtlas) {
        // The clear color alone is a valid frame (an empty grid), so this
        // runs even with no quads.
        guard let drawable = layer.nextDrawable() else { return }
        let viewSize = CGSize(
            width: CGFloat(drawable.texture.width) / layer.contentsScale,
            height: CGFloat(drawable.texture.height) / layer.contentsScale
        )
        _ = encode(
            into: drawable.texture,
            clear: clear,
            atlas: atlas,
            viewSize: viewSize,
            synchronize: false,
            present: drawable
        )
    }

    /// Encode the current quads into a texture. Shared by the on-screen path
    /// and the offscreen capture, so a screenshot renders exactly what the
    /// screen shows.
    private func encode(
        into target: MTLTexture,
        clear: SIMD4<Float>,
        atlas: LingXiaTerminalGlyphAtlas,
        viewSize: CGSize,
        synchronize: Bool,
        present: (any MTLDrawable)? = nil
    ) -> Bool {
        let descriptor = MTLRenderPassDescriptor()
        descriptor.colorAttachments[0].texture = target
        descriptor.colorAttachments[0].loadAction = .clear
        descriptor.colorAttachments[0].storeAction = .store
        descriptor.colorAttachments[0].clearColor = MTLClearColor(
            red: Double(clear.x), green: Double(clear.y), blue: Double(clear.z), alpha: 1
        )
        guard let commands = queue.makeCommandBuffer(),
              let encoder = commands.makeRenderCommandEncoder(descriptor: descriptor) else {
            return false
        }

        if !quads.isEmpty {
            let length = MemoryLayout<LingXiaTerminalQuad>.stride * quads.count
            if instanceBuffer == nil || instanceBuffer!.length < length {
                instanceBuffer = device.makeBuffer(length: max(length, 64 * 1024), options: .storageModeShared)
            }
            if let buffer = instanceBuffer {
                buffer.contents().copyMemory(from: quads, byteCount: length)
                var viewport = SIMD2<Float>(Float(viewSize.width), Float(viewSize.height))
                encoder.setRenderPipelineState(pipeline)
                encoder.setVertexBuffer(buffer, offset: 0, index: 0)
                encoder.setVertexBytes(&viewport, length: MemoryLayout<SIMD2<Float>>.size, index: 1)
                encoder.setFragmentTexture(atlas.texture, index: 0)
                encoder.setFragmentSamplerState(sampler, index: 0)
                encoder.drawPrimitives(
                    type: .triangleStrip,
                    vertexStart: 0,
                    vertexCount: 4,
                    instanceCount: quads.count
                )
            }
        }
        encoder.endEncoding()

        if synchronize {
            // A managed texture has to be blitted back before the CPU may
            // read it.
            if let blit = commands.makeBlitCommandEncoder() {
                blit.synchronize(resource: target)
                blit.endEncoding()
            }
        }
        if let present {
            commands.present(present)
        }
        commands.commit()
        if synchronize {
            commands.waitUntilCompleted()
        }
        return true
    }

    private func ensureAtlas(context: LingXiaTerminalRenderContext) -> LingXiaTerminalGlyphAtlas {
        let key = "\(context.font.fontName)|\(context.font.pointSize)|\(context.cellSize)|\(context.scale)|\(context.baseline)"
        if let atlas, key == atlasKey { return atlas }
        if let atlas {
            atlas.reset(font: context.font, cellSize: context.cellSize, baseline: context.baseline)
            syncShaper(with: atlas)
            atlasKey = key
            return atlas
        }
        let created = LingXiaTerminalGlyphAtlas(
            device: device,
            font: context.font,
            cellSize: context.cellSize,
            baseline: context.baseline
        )!
        atlas = created
        syncShaper(with: created)
        atlasKey = key
        return created
    }

    /// The shaper and the atlas must agree on the fonts, or a shaped glyph id
    /// would be rasterized from a different face.
    private func syncShaper(with atlas: LingXiaTerminalGlyphAtlas) {
        shaper.reset(
            regular: atlas.font,
            bold: atlas.boldFont,
            italic: atlas.italicFont,
            boldItalic: atlas.boldItalicFont
        )
    }

    private func solid(rect: CGRect, color: SIMD4<Float>) -> LingXiaTerminalQuad {
        LingXiaTerminalQuad(
            rect: SIMD4<Float>(Float(rect.minX), Float(rect.minY), Float(rect.width), Float(rect.height)),
            color: color,
            uv: SIMD4<Float>(0, 0, 0, 0),
            mode: SIMD4<Float>(0, 0, 0, 0)
        )
    }

    /// Cell rects land on whole pixels: a fractional cell width (7.2pt is
    /// typical) otherwise puts every column at a different subpixel offset,
    /// which resamples the glyph and reads as blur. Snapping both edges also
    /// makes backgrounds tile without seams.
    private static func cellRect(
        row: Int,
        col: Int,
        span: Int,
        context: LingXiaTerminalRenderContext
    ) -> CGRect {
        let scale = context.scale
        let left = snap(CGFloat(col) * context.cellSize.width, scale)
        let right = snap(CGFloat(col + max(span, 1)) * context.cellSize.width, scale)
        let top = snap(CGFloat(row) * context.cellSize.height, scale)
        let bottom = snap(CGFloat(row + 1) * context.cellSize.height, scale)
        return CGRect(x: left, y: top, width: right - left, height: bottom - top)
    }

    private static func snap(_ value: CGFloat, _ scale: CGFloat) -> CGFloat {
        (value * scale).rounded() / scale
    }

    /// Exponent applied to glyph coverage.
    ///
    /// Grayscale antialiasing computes coverage in a perceptual space but the
    /// GPU blends linearly, which thins light text on a dark background and
    /// fattens the reverse. Biasing the coverage per run restores the weight
    /// the rasterizer intended.
    private static func coverageExponent(text: SIMD4<Float>, background: SIMD4<Float>) -> Float {
        let textLuminance = 0.2126 * text.x + 0.7152 * text.y + 0.0722 * text.z
        let backgroundLuminance = 0.2126 * background.x + 0.7152 * background.y + 0.0722 * background.z
        return textLuminance >= backgroundLuminance ? 1 / 1.35 : 1.35
    }

    /// Engine colors are 0xRRGGBBAA; alpha 0 is the "default color" sentinel
    /// rather than transparency.
    private static func color(_ rgba: UInt32, fallbackAlpha: Float) -> SIMD4<Float> {
        let alpha = Float(rgba & 0xFF) / 255
        return SIMD4<Float>(
            Float((rgba >> 24) & 0xFF) / 255,
            Float((rgba >> 16) & 0xFF) / 255,
            Float((rgba >> 8) & 0xFF) / 255,
            alpha == 0 ? fallbackAlpha : alpha
        )
    }

    private static func color(_ color: NSColor) -> SIMD4<Float> {
        let rgb = color.usingColorSpace(.sRGB) ?? color
        return SIMD4<Float>(
            Float(rgb.redComponent),
            Float(rgb.greenComponent),
            Float(rgb.blueComponent),
            Float(rgb.alphaComponent)
        )
    }

    private static let shaderSource = """
    #include <metal_stdlib>
    using namespace metal;

    struct Quad {
        float4 rect;
        float4 color;
        float4 uv;
        float4 mode;
    };

    struct VertexOut {
        float4 position [[position]];
        float4 color;
        float2 uv;
        float mode;
        float coverageExponent;
    };

    vertex VertexOut lx_terminal_vertex(uint vid [[vertex_id]],
                                        uint iid [[instance_id]],
                                        const device Quad *quads [[buffer(0)]],
                                        constant float2 &viewport [[buffer(1)]]) {
        float2 corner = float2((vid == 1 || vid == 3) ? 1.0 : 0.0, (vid >= 2) ? 1.0 : 0.0);
        Quad quad = quads[iid];
        float2 point = quad.rect.xy + corner * quad.rect.zw;
        VertexOut out;
        out.position = float4(point.x / viewport.x * 2.0 - 1.0,
                              1.0 - point.y / viewport.y * 2.0,
                              0.0, 1.0);
        out.color = quad.color;
        out.uv = mix(quad.uv.xy, quad.uv.zw, corner);
        out.mode = quad.mode.x;
        out.coverageExponent = max(quad.mode.y, 0.01);
        return out;
    }

    fragment float4 lx_terminal_fragment(VertexOut in [[stage_in]],
                                         texture2d<float> atlas [[texture(0)]],
                                         sampler atlasSampler [[sampler(0)]]) {
        if (in.mode < 0.5) {
            return float4(in.color.rgb * in.color.a, in.color.a);
        }
        float4 texel = atlas.sample(atlasSampler, in.uv);
        if (in.mode < 1.5) {
            float coverage = pow(texel.a, in.coverageExponent) * in.color.a;
            return float4(in.color.rgb * coverage, coverage);
        }
        return texel * in.color.a;
    }
    """
}

extension LingXiaTerminalRenderContext {
    /// Distance from the top of a cell to the text baseline.
    var baselineFromTop: CGFloat { cellSize.height - baseline }
}
#endif
