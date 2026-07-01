import Foundation

extension Bundle {
    /// The Runner's own resource bundle, resolved robustly across build and
    /// packaging contexts — same problem as the SDK's `Bundle.lingxiaResources`.
    ///
    /// SwiftPM's generated `Bundle.module` resolves the module bundle via
    /// `Bundle.main.bundleURL` (the `.app` root once packaged) plus a compile-time
    /// `.build` path, neither of which exists in a shipped, code-signed `.app`
    /// where `lingxia package` places the bundle in `Contents/Resources` (the only
    /// codesign-clean location). Resolve `Contents/Resources` first so a fetched
    /// release Runner loads its resources; fall back to `Bundle.module` for tests
    /// and bare `swift build`. The fallback is autoclosure'd, so a shipped `.app`
    /// never evaluates `Bundle.module` and can't trip its fatalError.
    static let runnerResources: Bundle = resolveRunnerModuleBundle(
        named: "LingXiaRunner_LingXiaRunner.bundle",
        fallback: .module
    )
}

private func resolveRunnerModuleBundle(
    named name: String,
    fallback: @autoclosure () -> Bundle
) -> Bundle {
    if let resourceURL = Bundle.main.resourceURL {
        let candidate = resourceURL.appendingPathComponent(name)
        if let bundle = Bundle(url: candidate) {
            return bundle
        }
    }
    return fallback()
}
