/// A request to open an LxApp.
///
/// `userInfo` allows hosts to attach arbitrary metadata that is forwarded
/// back on `didOpen` events without interpretation by the SDK.
public struct LxAppOpenRequest: Codable, Sendable {
    public var appId: String
    public var path: String
    public var presentation: LxAppOpenPresentation
    public var panelId: String?
    public var pageWarmTtlMs: Int64?
    public var userInfo: [String: LxAppJSONValue]

    public init(
        appId: String,
        path: String = "/",
        presentation: LxAppOpenPresentation = .normal,
        panelId: String? = nil,
        pageWarmTtlMs: Int64? = nil,
        userInfo: [String: LxAppJSONValue] = [:]
    ) {
        self.appId = appId
        self.path = path
        self.presentation = presentation
        self.panelId = panelId
        self.pageWarmTtlMs = pageWarmTtlMs
        self.userInfo = userInfo
    }
}
