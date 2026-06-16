/// Placement of macOS traffic light buttons relative to the sidebar.
///
/// Only meaningful on macOS; ignored on iOS.
public enum LxAppTrafficLightPlacement: String, Codable, Sendable, CaseIterable {
    /// Traffic lights are inset into the sidebar header area (default).
    case sidebar
    /// Traffic lights sit in the toolbar, standard macOS position.
    case toolbar
    /// Traffic lights use the system default position (no customization).
    case system
}
