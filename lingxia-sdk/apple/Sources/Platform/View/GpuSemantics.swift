// Generated from contracts/view/schema.json; do not edit.
/// Semantic node roles, live-region kinds, event kinds and JSON keys of the shared projection.
public enum GpuSemantics {
    /// A semantic node's role.
    public enum Role: String, Sendable, CaseIterable {
        case button
        case checkBox = "check-box"
        case `switch`
        case radioGroup = "radio-group"
        case radioButton = "radio-button"
        case comboBox = "combo-box"
        case textField = "text-field"
        case scrollView = "scroll-view"
        case horizontalScrollView = "horizontal-scroll-view"
        case image
        case slider
        case text
        case link
        case heading
        case list
        case listItem = "list-item"
        case dialog
        case alert
        case status
        case grid
        case gridCell = "grid-cell"
        case listBox = "list-box"
        case option
        case menu
        case menuItem = "menu-item"
        case video
        case progressBar = "progress-bar"
        case alertDialog = "alert-dialog"
        case group
        case tooltip
        case tabList = "tab-list"
        case tab
        case tabPanel = "tab-panel"
        case toolbar
        case tree
        case treeItem = "tree-item"
        case row
        case menuItemCheckBox = "menu-item-check-box"
        case menuItemRadio = "menu-item-radio"
        case webView = "web-view"
    }

    /// How a live region announces a change.
    public enum LiveRegion: String, Sendable, CaseIterable {
        case polite
        case assertive
    }

    /// An event the projection raises.
    public enum Event: String, Sendable, CaseIterable {
        case announce
    }

    /// An action a node offers besides activation.
    public enum Action: String, Sendable, CaseIterable {
        case copy
        case cut
        case paste
        case setSelection = "set-selection"
    }

    /// A JSON key of a semantic node.
    public enum Key: String, Sendable, CaseIterable {
        case headingLevel = "heading_level"
        case liveRegion = "live_region"
        case position
        case textSelection = "text_selection"
        case selectable
        case pressed
        case selected
        case expanded
        case busy
        case scroll
        case description
        case invalid
        case required
        case controls
        case autocomplete
    }
}
