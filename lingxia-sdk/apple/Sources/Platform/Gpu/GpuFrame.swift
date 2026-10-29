// Generated from packages/lingxia-ui/contracts/schema.json; do not edit.
// The frame contract every Apple platform decodes: snapshot = FrameSnapshot, semanticNode = SemanticNode, automationRequest = AutomationRequest, automationElement = AutomationElement, automationQueryAll = AutomationQueryAll, automationTree = AutomationTree, automationFrame = AutomationFrame, automationCall = AutomationCall, automationResult = AutomationResult, inputRequest = InputRequest, inputReply = InputReply, copySelection = CopyReply, semanticRoots = SemanticRoots, textMenu = TextMenu.
import Foundation

/// Any JSON value, for contract fields typed `any`.
public enum GpuJSON: Decodable, Sendable, Equatable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([GpuJSON])
    case object([String: GpuJSON])

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() { self = .null }
        else if let value = try? container.decode(Bool.self) { self = .bool(value) }
        else if let value = try? container.decode(Double.self) { self = .number(value) }
        else if let value = try? container.decode(String.self) { self = .string(value) }
        else if let value = try? container.decode([GpuJSON].self) { self = .array(value) }
        else { self = .object(try container.decode([String: GpuJSON].self)) }
    }
}

/// `ScrollAxis` values.
public enum GpuFrameScrollAxis: String, Sendable {
    case horizontal = "horizontal"
    case vertical = "vertical"
}

/// `Keyboard` values.
public enum GpuFrameKeyboard: String, Sendable {
    case text = "text"
    case number = "number"
    case email = "email"
    case phone = "phone"
    case search = "search"
}

/// `Capitalize` values.
public enum GpuFrameCapitalize: String, Sendable {
    case none = "none"
    case sentences = "sentences"
    case words = "words"
    case characters = "characters"
}

/// `ReturnKey` values.
public enum GpuFrameReturnKey: String, Sendable {
    case newline = "newline"
    case done = "done"
    case go = "go"
    case next = "next"
    case previous = "previous"
    case search = "search"
    case send = "send"
}

/// `PickerKind` values.
public enum GpuFramePickerKind: String, Sendable {
    case date = "date"
    case time = "time"
}

/// `MenuAction` values.
public enum GpuFrameMenuAction: String, Sendable {
    case cut = "cut"
    case copy = "copy"
    case paste = "paste"
    case selectAll = "select_all"
    case undo = "undo"
    case redo = "redo"
}

/// `FrameSnapshot` in the frame contract.
public struct GpuFrameFrameSnapshot: Decodable, Sendable {
    public let generation: UInt64
    public let nodes: [GpuFrameNode]
    public let images: [GpuFrameImageSource]
    public let metrics: GpuFrameFrameMetrics
    public let feedback: GpuFrameFeedbackStyle

    enum CodingKeys: String, CodingKey {
        case generation = "generation"
        case nodes = "nodes"
        case images = "images"
        case metrics = "metrics"
        case feedback = "feedback"
    }
}

/// `ImageSource` in the frame contract.
public struct GpuFrameImageSource: Decodable, Sendable {
    public let source: String
    public let retryKey: String

    enum CodingKeys: String, CodingKey {
        case source = "source"
        case retryKey = "retry_key"
    }
}

/// `FrameMetrics` in the frame contract.
public struct GpuFrameFrameMetrics: Decodable, Sendable {
    public let generation: UInt64
    public let totalUs: UInt64
    public let viewUs: UInt64
    public let viewEvalUs: UInt64
    public let viewTransferUs: UInt64
    public let viewDecodeUs: UInt64
    public let wireValues: UInt64
    public let layoutUs: UInt64
    public let textUs: UInt64
    public let composeUs: UInt64
    public let buildUs: UInt64
    public let viewPasses: UInt32
    public let layoutPasses: UInt32
    public let nodes: UInt64
    public let paints: UInt64
    public let animationFrames: UInt64
    public let ghosts: UInt64?
    public let layoutMotions: UInt64?
    public let cost: GpuFrameFrameCost
    public let imageCacheBytes: UInt64?
    public let presenterSingleTarget: Bool?
    public let activations: UInt64?
    public let droppedActivations: UInt64?

    enum CodingKeys: String, CodingKey {
        case generation = "generation"
        case totalUs = "totalUs"
        case viewUs = "viewUs"
        case viewEvalUs = "viewEvalUs"
        case viewTransferUs = "viewTransferUs"
        case viewDecodeUs = "viewDecodeUs"
        case wireValues = "wireValues"
        case layoutUs = "layoutUs"
        case textUs = "textUs"
        case composeUs = "composeUs"
        case buildUs = "buildUs"
        case viewPasses = "viewPasses"
        case layoutPasses = "layoutPasses"
        case nodes = "nodes"
        case paints = "paints"
        case animationFrames = "animationFrames"
        case ghosts = "ghosts"
        case layoutMotions = "layoutMotions"
        case cost = "cost"
        case imageCacheBytes = "imageCacheBytes"
        case presenterSingleTarget = "presenterSingleTarget"
        case activations = "activations"
        case droppedActivations = "droppedActivations"
    }
}

/// `FrameCost` in the frame contract.
public struct GpuFrameFrameCost: Decodable, Sendable {
    public let nodes: UInt64
    public let paints: UInt64
    public let glyphs: UInt64
    public let vectorElements: UInt64
    public let layers: UInt64
    public let backdrops: UInt64
    public let shadows: UInt64
    public let images: UInt64
    public let imagePixels: UInt64
    public let outlineGlyphs: UInt64

    enum CodingKeys: String, CodingKey {
        case nodes = "nodes"
        case paints = "paints"
        case glyphs = "glyphs"
        case vectorElements = "vectorElements"
        case layers = "layers"
        case backdrops = "backdrops"
        case shadows = "shadows"
        case images = "images"
        case imagePixels = "imagePixels"
        case outlineGlyphs = "outlineGlyphs"
    }
}

/// `FeedbackStyle` in the frame contract.
public struct GpuFrameFeedbackStyle: Decodable, Sendable {
    public let pressOverlay: UInt32
    public let focusRing: UInt32
    public let focusRingWidth: Float
    public let selectionHandle: UInt32
    public let noticeBackground: UInt32
    public let noticeText: UInt32

    enum CodingKeys: String, CodingKey {
        case pressOverlay = "pressOverlay"
        case focusRing = "focusRing"
        case focusRingWidth = "focusRingWidth"
        case selectionHandle = "selectionHandle"
        case noticeBackground = "noticeBackground"
        case noticeText = "noticeText"
    }
}

/// `Node` in the frame contract.
public struct GpuFrameNode: Decodable, Sendable {
    public let identity: String
    public let tag: String
    public let testId: String
    public let text: String
    public let bounds: [Float]
    public let visibleBounds: [Float]
    public let selectable: String?
    public let textSelection: GpuFrameInputGeometry?
    public let selectionRange: [UInt32]?
    public let hitRects: [[Float]]?
    public let clips: [GpuFrameClip]?
    public let scroll: GpuFrameScroll?
    public let onClick: UInt64?
    public let checked: Bool?
    public let range: GpuFrameSlider?
    public let plot: GpuFramePlot?
    public let radioGroup: String?
    public let scrollParent: String?
    public let accessibilityLabel: String?
    public let accessibilityHidden: Bool?
    public let role: String?
    public let headingLevel: UInt8?
    public let liveRegion: String?
    public let select: GpuFrameSelect?
    public let buttonContent: String?
    public let radii: [[Float]]?
    public let radius: Float
    public let contentClip: GpuFrameClip?
    public let controlBounds: [Float]?
    public let input: GpuFrameInput?
    public let clipsChildren: Bool?
    public let pressedStyle: Bool?
    public let focusStyle: Bool?
    public let order: UInt32

    enum CodingKeys: String, CodingKey {
        case identity = "identity"
        case tag = "tag"
        case testId = "test_id"
        case text = "text"
        case bounds = "bounds"
        case visibleBounds = "visible_bounds"
        case selectable = "selectable"
        case textSelection = "text_selection"
        case selectionRange = "selection_range"
        case hitRects = "hit_rects"
        case clips = "clips"
        case scroll = "scroll"
        case onClick = "on_click"
        case checked = "checked"
        case range = "range"
        case plot = "plot"
        case radioGroup = "radio_group"
        case scrollParent = "scroll_parent"
        case accessibilityLabel = "accessibility_label"
        case accessibilityHidden = "accessibility_hidden"
        case role = "role"
        case headingLevel = "heading_level"
        case liveRegion = "live_region"
        case select = "select"
        case buttonContent = "button_content"
        case radii = "radii"
        case radius = "radius"
        case contentClip = "content_clip"
        case controlBounds = "control_bounds"
        case input = "input"
        case clipsChildren = "clips_children"
        case pressedStyle = "pressed_style"
        case focusStyle = "focus_style"
        case order = "order"
    }
}

/// `Clip` in the frame contract.
public struct GpuFrameClip: Decodable, Sendable {
    public let radii: [[Float]]?
    public let bounds: [Float]
    public let radius: Float
    public let transform: [Double]?

    enum CodingKeys: String, CodingKey {
        case radii = "radii"
        case bounds = "bounds"
        case radius = "radius"
        case transform = "transform"
    }
}

/// `Scroll` in the frame contract.
public struct GpuFrameScroll: Decodable, Sendable {
    public let rtl: Bool?
    public let contain: Bool?
    public let axis: String
    public let onReachStart: UInt64?
    public let onReachEnd: UInt64?
    public let id: String
    public let parent: String?
    public let offset: Float
    public let maxOffset: Float

    enum CodingKeys: String, CodingKey {
        case rtl = "rtl"
        case contain = "contain"
        case axis = "axis"
        case onReachStart = "on_reach_start"
        case onReachEnd = "on_reach_end"
        case id = "id"
        case parent = "parent"
        case offset = "offset"
        case maxOffset = "max_offset"
    }
}

/// `InputGeometry` in the frame contract.
public struct GpuFrameInputGeometry: Decodable, Sendable {
    public let anchor: [Float]
    public let caret: [Float]
    public let contentBounds: [Float]
    public let selection: [[Float]]
    public let composition: [[Float]]

    enum CodingKeys: String, CodingKey {
        case anchor = "anchor"
        case caret = "caret"
        case contentBounds = "content_bounds"
        case selection = "selection"
        case composition = "composition"
    }
}

/// `Slider` in the frame contract.
public struct GpuFrameSlider: Decodable, Sendable {
    public let min: Double
    public let max: Double
    public let step: Double
    public let value: Double
    public let inset: Float
    public let enabled: Bool
    public let onChange: UInt64?

    enum CodingKeys: String, CodingKey {
        case min = "min"
        case max = "max"
        case step = "step"
        case value = "value"
        case inset = "inset"
        case enabled = "enabled"
        case onChange = "on_change"
    }
}

/// `Plot` in the frame contract.
public struct GpuFramePlot: Decodable, Sendable {
    public let kind: String
    public let values: [Double]
    public let domain: [Double]
    public let pieLayout: GpuFramePieLayout
    public let selectedIndex: Int64
    public let onSelect: UInt64?

    enum CodingKeys: String, CodingKey {
        case kind = "kind"
        case values = "values"
        case domain = "domain"
        case pieLayout = "pie_layout"
        case selectedIndex = "selected_index"
        case onSelect = "on_select"
    }
}

/// `PieLayout` in the frame contract.
public struct GpuFramePieLayout: Decodable, Sendable {
    public let center: [Double]
    public let innerRadius: Double
    public let outerRadius: Double
    public let emphasisScale: Double

    enum CodingKeys: String, CodingKey {
        case center = "center"
        case innerRadius = "innerRadius"
        case outerRadius = "outerRadius"
        case emphasisScale = "emphasisScale"
    }
}

/// `Select` in the frame contract.
public struct GpuFrameSelect: Decodable, Sendable {
    public let options: [GpuFrameSelectOption]
    public let selected: UInt64?
    public let onSelect: UInt64?

    enum CodingKeys: String, CodingKey {
        case options = "options"
        case selected = "selected"
        case onSelect = "on_select"
    }
}

/// `SelectOption` in the frame contract.
public struct GpuFrameSelectOption: Decodable, Sendable {
    public let value: String
    public let label: String
    public let enabled: Bool

    enum CodingKeys: String, CodingKey {
        case value = "value"
        case label = "label"
        case enabled = "enabled"
    }
}

/// `Input` in the frame contract.
public struct GpuFrameInput: Decodable, Sendable {
    public let value: String
    public let placeholder: String
    public let secure: Bool
    public let keyboard: String
    public let autoFocus: Bool
    public let submitOnEnter: Bool
    public let enabled: Bool
    public let onInput: UInt64?
    public let onSubmit: UInt64?
    public let maxLength: UInt32?
    public let autoComplete: String?
    public let enterKeyHint: String?
    public let autoCapitalize: String?
    public let spellCheck: Bool?
    public let min: GpuJSON?
    public let max: GpuJSON?
    public let step: Double?
    public let picker: String?
    public let edit: GpuFrameEdit?
    public let editToken: UInt64?
    public let geometry: GpuFrameInputGeometry?
    public let scrollOffset: [Float]
    public let ime: GpuFrameIme
    public let raiseKeyboard: Bool

    enum CodingKeys: String, CodingKey {
        case value = "value"
        case placeholder = "placeholder"
        case secure = "secure"
        case keyboard = "keyboard"
        case autoFocus = "auto_focus"
        case submitOnEnter = "submit_on_enter"
        case enabled = "enabled"
        case onInput = "on_input"
        case onSubmit = "on_submit"
        case maxLength = "max_length"
        case autoComplete = "auto_complete"
        case enterKeyHint = "enter_key_hint"
        case autoCapitalize = "auto_capitalize"
        case spellCheck = "spell_check"
        case min = "min"
        case max = "max"
        case step = "step"
        case picker = "picker"
        case edit = "edit"
        case editToken = "edit_token"
        case geometry = "geometry"
        case scrollOffset = "scroll_offset"
        case ime = "ime"
        case raiseKeyboard = "raise_keyboard"
    }
}

/// `Edit` in the frame contract.
public struct GpuFrameEdit: Decodable, Sendable {
    public let text: String
    public let selection: [UInt32]
    public let composition: [UInt32]?
    public let focused: Bool
    public let canUndo: Bool
    public let canRedo: Bool

    enum CodingKeys: String, CodingKey {
        case text = "text"
        case selection = "selection"
        case composition = "composition"
        case focused = "focused"
        case canUndo = "can_undo"
        case canRedo = "can_redo"
    }
}

/// `Ime` in the frame contract.
public struct GpuFrameIme: Decodable, Sendable {
    public let keyboard: String
    public let secure: Bool
    public let multiline: Bool
    public let capitalize: String
    public let suggestions: Bool
    public let returnKey: String
    public let returnInsertsNewline: Bool
    public let hideOnSubmit: Bool
    public let picker: String?
    public let autoComplete: String?
    public let maxLength: UInt32?

    enum CodingKeys: String, CodingKey {
        case keyboard = "keyboard"
        case secure = "secure"
        case multiline = "multiline"
        case capitalize = "capitalize"
        case suggestions = "suggestions"
        case returnKey = "return_key"
        case returnInsertsNewline = "return_inserts_newline"
        case hideOnSubmit = "hide_on_submit"
        case picker = "picker"
        case autoComplete = "auto_complete"
        case maxLength = "max_length"
    }
}

/// `SemanticNode` in the frame contract.
public struct GpuFrameSemanticNode: Decodable, Sendable {
    public let id: UInt32
    public let index: UInt64
    public let role: String
    public let name: String
    public let text: String
    public let testId: String
    public let bounds: [Float]
    public let visible: Bool
    public let enabled: Bool
    public let focusable: Bool
    public let clickable: Bool
    public let focused: Bool
    public let checked: Bool?
    public let scrollable: Bool
    public let range: GpuFrameSemanticRange?
    public let textField: GpuFrameTextField?
    public let parent: UInt32?
    public let children: [UInt32]
    public let position: [UInt64]?
    public let actions: [GpuFrameSemanticAction]
    public let headingLevel: UInt8?
    public let liveRegion: String?
    public let textSelection: [UInt32]?
    public let pressed: Bool?
    public let selected: Bool?
    public let expanded: Bool?
    public let busy: Bool?
    public let value: String?
    public let scroll: GpuFrameScrollPosition?

    enum CodingKeys: String, CodingKey {
        case id = "id"
        case index = "index"
        case role = "role"
        case name = "name"
        case text = "text"
        case testId = "test_id"
        case bounds = "bounds"
        case visible = "visible"
        case enabled = "enabled"
        case focusable = "focusable"
        case clickable = "clickable"
        case focused = "focused"
        case checked = "checked"
        case scrollable = "scrollable"
        case range = "range"
        case textField = "text_field"
        case parent = "parent"
        case children = "children"
        case position = "position"
        case actions = "actions"
        case headingLevel = "heading_level"
        case liveRegion = "live_region"
        case textSelection = "text_selection"
        case pressed = "pressed"
        case selected = "selected"
        case expanded = "expanded"
        case busy = "busy"
        case value = "value"
        case scroll = "scroll"
    }
}

/// `SemanticRange` in the frame contract.
public struct GpuFrameSemanticRange: Decodable, Sendable {
    public let min: Double
    public let max: Double
    public let value: Double

    enum CodingKeys: String, CodingKey {
        case min = "min"
        case max = "max"
        case value = "value"
    }
}

/// `TextField` in the frame contract.
public struct GpuFrameTextField: Decodable, Sendable {
    public let text: String
    public let hint: String
    public let secure: Bool
    public let multiline: Bool
    public let selection: [UInt32]?

    enum CodingKeys: String, CodingKey {
        case text = "text"
        case hint = "hint"
        case secure = "secure"
        case multiline = "multiline"
        case selection = "selection"
    }
}

/// `ScrollPosition` in the frame contract.
public struct GpuFrameScrollPosition: Decodable, Sendable {
    public let offset: Float
    public let maxOffset: Float
    public let horizontal: Bool

    enum CodingKeys: String, CodingKey {
        case offset = "offset"
        case maxOffset = "max_offset"
        case horizontal = "horizontal"
    }
}

/// `SemanticAction` in the frame contract.
public struct GpuFrameSemanticAction: Decodable, Sendable {
    public let kind: String
    public let index: UInt64?
    public let label: String?

    enum CodingKeys: String, CodingKey {
        case kind = "kind"
        case index = "index"
        case label = "label"
    }
}

/// `AutomationRequest` in the frame contract.
public struct GpuFrameAutomationRequest: Decodable, Sendable {
    public let op: String
    public let css: String?
    public let index: UInt64?
    public let all: Bool?
    public let maxText: UInt64?
    public let action: GpuFrameAutomationAction?
    public let name: String?
    public let args: GpuJSON?
    public let call: UInt64?

    enum CodingKeys: String, CodingKey {
        case op = "op"
        case css = "css"
        case index = "index"
        case all = "all"
        case maxText = "maxText"
        case action = "action"
        case name = "name"
        case args = "args"
        case call = "call"
    }
}

/// `InputRequest` in the frame contract.
public struct GpuFrameInputRequest: Decodable, Sendable {
    public let op: String
    public let identity: String?
    public let generation: UInt64?
    public let token: UInt64?
    public let point: [Float]?
    public let edge: [Int64]?
    public let word: Bool?
    public let edits: [GpuFrameInputEdit]?
    public let submit: Bool?

    enum CodingKeys: String, CodingKey {
        case op = "op"
        case identity = "identity"
        case generation = "generation"
        case token = "token"
        case point = "point"
        case edge = "edge"
        case word = "word"
        case edits = "edits"
        case submit = "submit"
    }
}

/// `InputEdit` in the frame contract.
public struct GpuFrameInputEdit: Decodable, Sendable {
    public let op: String
    public let anchor: UInt32?
    public let focus: UInt32?
    public let start: UInt32?
    public let end: UInt32?
    public let text: String?
    public let cursor: Int64?
    public let composing: Bool?
    public let before: UInt32?
    public let after: UInt32?
    public let codePoints: Bool?

    enum CodingKeys: String, CodingKey {
        case op = "op"
        case anchor = "anchor"
        case focus = "focus"
        case start = "start"
        case end = "end"
        case text = "text"
        case cursor = "cursor"
        case composing = "composing"
        case before = "before"
        case after = "after"
        case codePoints = "code_points"
    }
}

/// `AutomationCall` in the frame contract.
public struct GpuFrameAutomationCall: Decodable, Sendable {
    public let call: UInt64

    enum CodingKeys: String, CodingKey {
        case call = "call"
    }
}

/// `AutomationResult` in the frame contract.
public struct GpuFrameAutomationResult: Decodable, Sendable {
    public let done: Bool
    public let value: GpuJSON?

    enum CodingKeys: String, CodingKey {
        case done = "done"
        case value = "value"
    }
}

/// `AutomationAction` in the frame contract.
public struct GpuFrameAutomationAction: Decodable, Sendable {
    public let kind: String
    public let force: Bool?
    public let text: String?
    public let key: String?
    public let value: String?

    enum CodingKeys: String, CodingKey {
        case kind = "kind"
        case force = "force"
        case text = "text"
        case key = "key"
        case value = "value"
    }
}

/// `AutomationElement` in the frame contract.
public struct GpuFrameAutomationElement: Decodable, Sendable {
    public let exists: Bool
    public let renderer: String
    public let index: UInt64
    public let count: UInt64
    public let tag: String?
    public let type: String?
    public let id: String?
    public let name: String?
    public let role: String?
    public let ariaLabel: String?
    public let placeholder: String?
    public let visible: Bool
    public let inViewport: Bool
    public let enabled: Bool
    public let editable: Bool
    public let checked: Bool?
    public let text: String?
    public let textTruncated: Bool?
    public let value: String?
    public let valueTruncated: Bool?
    public let rect: GpuFrameAutomationRect?
    public let attributes: GpuJSON?

    enum CodingKeys: String, CodingKey {
        case exists = "exists"
        case renderer = "renderer"
        case index = "index"
        case count = "count"
        case tag = "tag"
        case type = "type"
        case id = "id"
        case name = "name"
        case role = "role"
        case ariaLabel = "aria_label"
        case placeholder = "placeholder"
        case visible = "visible"
        case inViewport = "inViewport"
        case enabled = "enabled"
        case editable = "editable"
        case checked = "checked"
        case text = "text"
        case textTruncated = "text_truncated"
        case value = "value"
        case valueTruncated = "value_truncated"
        case rect = "rect"
        case attributes = "attributes"
    }
}

/// `AutomationQueryAll` in the frame contract.
public struct GpuFrameAutomationQueryAll: Decodable, Sendable {
    public let count: UInt64
    public let renderer: String
    public let items: [GpuFrameAutomationElement]

    enum CodingKeys: String, CodingKey {
        case count = "count"
        case renderer = "renderer"
        case items = "items"
    }
}

/// `AutomationFrame` in the frame contract.
public struct GpuFrameAutomationFrame: Decodable, Sendable {
    public let renderer: String
    public let page: String
    public let commit: UInt64
    public let firstPresentAt: UInt64?
    public let lastPresentAt: UInt64?
    public let frames: UInt64
    public let workerQueueUs: GpuFrameQueueTimes
    public let evalMs: GpuFrameEvalTimes
    public let imageCacheBytes: UInt64
    public let imageCacheEntries: UInt64
    public let animationFrames: UInt64
    public let wakeups: UInt64
    public let reveal: GpuJSON

    enum CodingKeys: String, CodingKey {
        case renderer = "renderer"
        case page = "page"
        case commit = "commit"
        case firstPresentAt = "firstPresentAt"
        case lastPresentAt = "lastPresentAt"
        case frames = "frames"
        case workerQueueUs = "workerQueueUs"
        case evalMs = "evalMs"
        case imageCacheBytes = "imageCacheBytes"
        case imageCacheEntries = "imageCacheEntries"
        case animationFrames = "animationFrames"
        case wakeups = "wakeups"
        case reveal = "reveal"
    }
}

/// `QueueTimes` in the frame contract.
public struct GpuFrameQueueTimes: Decodable, Sendable {
    public let last: UInt64
    public let p50: UInt64
    public let max: UInt64

    enum CodingKeys: String, CodingKey {
        case last = "last"
        case p50 = "p50"
        case max = "max"
    }
}

/// `EvalTimes` in the frame contract.
public struct GpuFrameEvalTimes: Decodable, Sendable {
    public let last: Double
    public let max: Double

    enum CodingKeys: String, CodingKey {
        case last = "last"
        case max = "max"
    }
}

/// `AutomationRect` in the frame contract.
public struct GpuFrameAutomationRect: Decodable, Sendable {
    public let left: Float
    public let top: Float
    public let width: Float
    public let height: Float
    public let right: Float
    public let bottom: Float
    public let centerX: Float
    public let centerY: Float
    public let viewportWidth: Float
    public let viewportHeight: Float

    enum CodingKeys: String, CodingKey {
        case left = "left"
        case top = "top"
        case width = "width"
        case height = "height"
        case right = "right"
        case bottom = "bottom"
        case centerX = "center_x"
        case centerY = "center_y"
        case viewportWidth = "viewport_width"
        case viewportHeight = "viewport_height"
    }
}

/// `AutomationTree` in the frame contract.
public struct GpuFrameAutomationTree: Decodable, Sendable {
    public let renderer: String
    public let nodes: [GpuFrameAutomationTreeNode]

    enum CodingKeys: String, CodingKey {
        case renderer = "renderer"
        case nodes = "nodes"
    }
}

/// `AutomationTreeNode` in the frame contract.
public struct GpuFrameAutomationTreeNode: Decodable, Sendable {
    public let role: String
    public let name: String
    public let testId: String?
    public let bounds: [Float]
    public let disabled: Bool?
    public let focused: Bool?
    public let offscreen: Bool?
    public let checked: Bool?
    public let pressed: Bool?
    public let selected: Bool?
    public let expanded: Bool?
    public let busy: Bool?
    public let level: UInt8?
    public let value: GpuJSON?
    public let scroll: [Float]?
    public let children: [GpuFrameAutomationTreeNode]?

    enum CodingKeys: String, CodingKey {
        case role = "role"
        case name = "name"
        case testId = "testId"
        case bounds = "bounds"
        case disabled = "disabled"
        case focused = "focused"
        case offscreen = "offscreen"
        case checked = "checked"
        case pressed = "pressed"
        case selected = "selected"
        case expanded = "expanded"
        case busy = "busy"
        case level = "level"
        case value = "value"
        case scroll = "scroll"
        case children = "children"
    }
}

/// `InputReply` in the frame contract.
public struct GpuFrameInputReply: Decodable, Sendable {
    public let token: UInt64?
    public let state: GpuFrameEditorState?
    public let needsViewRender: Bool?
    public let movedTo: String?
    public let selection: [UInt32]?

    enum CodingKeys: String, CodingKey {
        case token = "token"
        case state = "state"
        case needsViewRender = "needs_view_render"
        case movedTo = "moved_to"
        case selection = "selection"
    }
}

/// `EditorState` in the frame contract.
public struct GpuFrameEditorState: Decodable, Sendable {
    public let text: String
    public let selection: [UInt32]
    public let composition: [UInt32]?
    public let focused: Bool
    public let canUndo: Bool
    public let canRedo: Bool

    enum CodingKeys: String, CodingKey {
        case text = "text"
        case selection = "selection"
        case composition = "composition"
        case focused = "focused"
        case canUndo = "can_undo"
        case canRedo = "can_redo"
    }
}

/// `CopyReply` in the frame contract.
public struct GpuFrameCopyReply: Decodable, Sendable {
    public let text: String?

    enum CodingKeys: String, CodingKey {
        case text = "text"
    }
}

/// `SemanticRoots` in the frame contract.
public struct GpuFrameSemanticRoots: Decodable, Sendable {
    public let roots: [UInt32]
    public let inputFocus: UInt32?
    public let accessibilityFocus: UInt32?

    enum CodingKeys: String, CodingKey {
        case roots = "roots"
        case inputFocus = "inputFocus"
        case accessibilityFocus = "accessibilityFocus"
    }
}

/// `TextMenu` in the frame contract.
public struct GpuFrameTextMenu: Decodable, Sendable {
    public let target: GpuFrameTextMenuTarget
    public let anchor: [Float]
    public let actions: [String]

    enum CodingKeys: String, CodingKey {
        case target = "target"
        case anchor = "anchor"
        case actions = "actions"
    }
}

/// `TextMenuTarget` in the frame contract.
public struct GpuFrameTextMenuTarget: Decodable, Sendable {
    public let kind: String
    public let identity: String?

    enum CodingKeys: String, CodingKey {
        case kind = "kind"
        case identity = "identity"
    }
}

/// The core's editor limits; an input mirror holds itself to them, since a rejected edit fails the session.
public enum GpuFrameInputLimits {
    public static let editorTextBytes = 65536
    public static let inputRequestBytes = 524288
    public static let inputBatchEdits = 64
    public static let clipboardItems = 128
}

/// A JSON key named at run time, for request encoders.
public struct GpuFrameKey: CodingKey {
    public let stringValue: String
    public var intValue: Int? { nil }
    public init(_ string: String) { stringValue = string }
    public init?(stringValue: String) { self.stringValue = stringValue }
    public init?(intValue: Int) { nil }
}

/// A `InputRequest` the platform sends to the core.
public enum GpuFrameInputRequestMessage: Encodable, Sendable {
    case dragHitTest(identity: String, generation: UInt64, token: UInt64, point: [Float], edge: [Int64])
    case hitTest(identity: String, generation: UInt64, point: [Float], word: Bool)
    case focus(identity: String, generation: UInt64)
    case query(identity: String, token: UInt64)
    case blur(identity: String, token: UInt64)
    case edit(identity: String, token: UInt64, edits: [GpuFrameInputEditMessage], submit: Bool)

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: GpuFrameKey.self)
        switch self {
        case .dragHitTest(let identity, let generation, let token, let point, let edge):
            try container.encode("drag_hit_test", forKey: GpuFrameKey("op"))
            try container.encode(identity, forKey: GpuFrameKey("identity"))
            try container.encode(generation, forKey: GpuFrameKey("generation"))
            try container.encode(token, forKey: GpuFrameKey("token"))
            try container.encode(point, forKey: GpuFrameKey("point"))
            try container.encode(edge, forKey: GpuFrameKey("edge"))
        case .hitTest(let identity, let generation, let point, let word):
            try container.encode("hit_test", forKey: GpuFrameKey("op"))
            try container.encode(identity, forKey: GpuFrameKey("identity"))
            try container.encode(generation, forKey: GpuFrameKey("generation"))
            try container.encode(point, forKey: GpuFrameKey("point"))
            try container.encode(word, forKey: GpuFrameKey("word"))
        case .focus(let identity, let generation):
            try container.encode("focus", forKey: GpuFrameKey("op"))
            try container.encode(identity, forKey: GpuFrameKey("identity"))
            try container.encode(generation, forKey: GpuFrameKey("generation"))
        case .query(let identity, let token):
            try container.encode("query", forKey: GpuFrameKey("op"))
            try container.encode(identity, forKey: GpuFrameKey("identity"))
            try container.encode(token, forKey: GpuFrameKey("token"))
        case .blur(let identity, let token):
            try container.encode("blur", forKey: GpuFrameKey("op"))
            try container.encode(identity, forKey: GpuFrameKey("identity"))
            try container.encode(token, forKey: GpuFrameKey("token"))
        case .edit(let identity, let token, let edits, let submit):
            try container.encode("edit", forKey: GpuFrameKey("op"))
            try container.encode(identity, forKey: GpuFrameKey("identity"))
            try container.encode(token, forKey: GpuFrameKey("token"))
            try container.encode(edits, forKey: GpuFrameKey("edits"))
            try container.encode(submit, forKey: GpuFrameKey("submit"))
        }
    }
}

/// A `InputEdit` the platform sends to the core.
public enum GpuFrameInputEditMessage: Encodable, Sendable {
    case selection(anchor: UInt32, focus: UInt32)
    case composition(start: UInt32, end: UInt32)
    case replace(text: String, cursor: Int64, composing: Bool)
    case finishComposition
    case undo
    case redo
    case deleteSurrounding(before: UInt32, after: UInt32, codePoints: Bool)

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: GpuFrameKey.self)
        switch self {
        case .selection(let anchor, let focus):
            try container.encode("selection", forKey: GpuFrameKey("op"))
            try container.encode(anchor, forKey: GpuFrameKey("anchor"))
            try container.encode(focus, forKey: GpuFrameKey("focus"))
        case .composition(let start, let end):
            try container.encode("composition", forKey: GpuFrameKey("op"))
            try container.encode(start, forKey: GpuFrameKey("start"))
            try container.encode(end, forKey: GpuFrameKey("end"))
        case .replace(let text, let cursor, let composing):
            try container.encode("replace", forKey: GpuFrameKey("op"))
            try container.encode(text, forKey: GpuFrameKey("text"))
            try container.encode(cursor, forKey: GpuFrameKey("cursor"))
            try container.encode(composing, forKey: GpuFrameKey("composing"))
        case .finishComposition:
            try container.encode("finish_composition", forKey: GpuFrameKey("op"))
        case .undo:
            try container.encode("undo", forKey: GpuFrameKey("op"))
        case .redo:
            try container.encode("redo", forKey: GpuFrameKey("op"))
        case .deleteSurrounding(let before, let after, let codePoints):
            try container.encode("delete_surrounding", forKey: GpuFrameKey("op"))
            try container.encode(before, forKey: GpuFrameKey("before"))
            try container.encode(after, forKey: GpuFrameKey("after"))
            try container.encode(codePoints, forKey: GpuFrameKey("code_points"))
        }
    }
}
