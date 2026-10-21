import AppKit

/// What the toolbar's inspector button shows. WebKit has no public call that
/// opens the Web Inspector, and puts Inspect Element in a page's context menu
/// only through private API, so the way in is Safari's Develop menu: this
/// says how, and opens Safari.
@MainActor
final class InspectorHelpViewController: NSViewController {
    /// Called once Safari is asked to open, so the popover can close.
    var onOpenSafari: (() -> Void)?

    override func loadView() {
        let mac = Host.current().localizedName ?? "this Mac"
        let title = NSTextField(labelWithString: "Web Inspector")
        title.font = .boldSystemFont(ofSize: 13)
        let steps = NSTextField(wrappingLabelWithString: """
            Safari inspects this Runner's pages: Safari ▸ Develop ▸ \(mac) ▸ \
            LingXia Runner, then pick the page.

            No Develop menu? Safari ▸ Settings ▸ Advanced ▸ Show features for \
            web developers.

            Network traffic: lxdev network devtools
            """)
        steps.font = .systemFont(ofSize: 12)
        steps.preferredMaxLayoutWidth = 300
        let open = NSButton(title: "Open Safari", target: self, action: #selector(openSafari))
        open.bezelStyle = .rounded
        open.keyEquivalent = "\r"

        let stack = NSStackView(views: [title, steps, open])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 10
        stack.translatesAutoresizingMaskIntoConstraints = false
        let container = NSView()
        container.addSubview(stack)
        NSLayoutConstraint.activate([
            steps.widthAnchor.constraint(equalToConstant: 300),
            stack.topAnchor.constraint(equalTo: container.topAnchor, constant: 14),
            stack.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -14),
            stack.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 16),
            stack.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -16),
        ])
        // A popover takes its size from here; a wrapping label has none of
        // its own until its width is fixed.
        container.layoutSubtreeIfNeeded()
        preferredContentSize = container.fittingSize
        view = container
    }

    @objc private func openSafari() {
        if let safari = NSWorkspace.shared.urlForApplication(withBundleIdentifier: "com.apple.Safari") {
            NSWorkspace.shared.openApplication(at: safari, configuration: NSWorkspace.OpenConfiguration())
        }
        onOpenSafari?()
    }
}
