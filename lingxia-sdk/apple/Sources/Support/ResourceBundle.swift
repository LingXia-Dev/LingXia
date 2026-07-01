import Foundation

extension Bundle {
    /// The SDK's resource bundle, resolved robustly across build and packaging
    /// contexts.
    ///
    /// SwiftPM's generated `Bundle.module` resolves the module bundle only via
    /// `Bundle.main.bundleURL` — which becomes the `.app` root once the binary is
    /// packaged into an app — plus a compile-time `.build` path. Neither location
    /// exists in a shipped, code-signed `.app`: `lingxia package` places the
    /// bundle in `Contents/Resources`, the only codesign-clean spot (a bundle at
    /// the `.app` root fails signing with "unsealed contents present in the bundle
    /// root"). So every released macOS app built from this SDK — the Runner, the
    /// showcase, and any `lingxia new` app — would crash on `Bundle.module`
    /// (`could not load resource bundle`) on a machine that isn't the build
    /// machine.
    ///
    /// Resolve `Contents/Resources` first (via `Bundle.main.resourceURL`) so a
    /// released app loads its resources with a valid signature, then fall back to
    /// `Bundle.module` for contexts where that path is correct (unit tests, a bare
    /// `swift build`, or an Xcode-built consumer). The fallback is autoclosure'd,
    /// so a shipped `.app` — where the primary lookup succeeds — never evaluates
    /// `Bundle.module` and can't trip its fatalError.
    static let lingxiaResources: Bundle = resolveLingxiaModuleBundle(
        named: "lingxia_lingxia.bundle",
        fallback: .module
    )
}

private func resolveLingxiaModuleBundle(
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
