/// How an LxApp should be presented when opened.
public enum LxAppOpenPresentation: String, Codable, Sendable, CaseIterable {
    /// Normal full-content presentation.
    case normal
    /// Presented in a side panel.
    case panel
}
