#if os(iOS)
import UIKit
import OSLog
import CLingXiaRustAPI

/// The phone host drawer — the subtle, understated host entry. Three zones:
///   • identity (top):  product icon · name · version (from `app.json`)
///   • activators (mid): the **declared** sidebar activators (`ui.json`), with
///     their real (PDF) icons, filtered by `platforms` (terminal is dropped on
///     iOS), plus the in-app Browser
///   • utility (bottom): settings · downloads · collapse
/// No lxapp switcher (lxapps are a stack); no floating bubble — an app that wants
/// to feature a surface adds its own button via the JS API.
@MainActor
final class LxAppHostDrawer: UIView {
    private static let log = OSLog(subsystem: "LingXia", category: "HostDrawer")

    /// Tapped a declared activator — carries its action so the host can run it.
    var onActivator: ((ActivatorTap) -> Void)?
    /// Tapped the in-app Browser entry.
    var onOpenBrowser: (() -> Void)?
    /// Tapped a host baseline entry (settings / downloads).
    var onHostEntry: ((HostEntry) -> Void)?

    enum HostEntry { case settings, downloads }
    struct ActivatorTap { let id: String; let surface: String; let action: String }

    private let scrim = UIView()
    private let panel = UIView()
    private let iconView = UIImageView()
    private let nameLabel = UILabel()
    private let versionLabel = UILabel()
    private let actsStack = UIStackView()
    private var panelLeading: NSLayoutConstraint!
    private var panelWidth: NSLayoutConstraint!
    private(set) var isOpen = false

    private var appConfig: LxAppGeneratedAppConfig?
    private var uiConfig: LxAppUIConfig?
    private var uiConfigURL: URL?

    override init(frame: CGRect) {
        super.init(frame: frame)
        isHidden = true
        translatesAutoresizingMaskIntoConstraints = false
        if let bundle = try? LxAppAppUIBundleLoader.loadFromMainBundle() {
            appConfig = bundle.app; uiConfig = bundle.ui; uiConfigURL = bundle.uiURL
        }
        buildScrim()
        buildPanel()
    }

    required init?(coder: NSCoder) { fatalError() }

    override func layoutSubviews() {
        super.layoutSubviews()
        panelWidth.constant = min(300, bounds.width * 0.78)
    }

    // MARK: - Open / close

    func toggle() { isOpen ? close() : open() }

    func open() {
        guard !isOpen else { return }
        isOpen = true
        isHidden = false
        reload()
        layoutIfNeeded()
        panelLeading.constant = 0
        UIView.animate(withDuration: 0.30, delay: 0, usingSpringWithDamping: 0.9, initialSpringVelocity: 0.3, options: [.curveEaseOut]) {
            self.scrim.alpha = 1
            self.layoutIfNeeded()
        }
    }

    func close() {
        guard isOpen else { return }
        isOpen = false
        panelLeading.constant = -panelWidth.constant
        UIView.animate(withDuration: 0.22, delay: 0, options: [.curveEaseIn]) {
            self.scrim.alpha = 0
            self.layoutIfNeeded()
        } completion: { _ in
            if !self.isOpen { self.isHidden = true }
        }
    }

    // MARK: - Build

    private func buildScrim() {
        scrim.translatesAutoresizingMaskIntoConstraints = false
        scrim.backgroundColor = UIColor.black.withAlphaComponent(0.3)
        scrim.alpha = 0
        addSubview(scrim)
        NSLayoutConstraint.activate([
            scrim.topAnchor.constraint(equalTo: topAnchor),
            scrim.leadingAnchor.constraint(equalTo: leadingAnchor),
            scrim.trailingAnchor.constraint(equalTo: trailingAnchor),
            scrim.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
        scrim.addGestureRecognizer(UITapGestureRecognizer(target: self, action: #selector(scrimTapped)))
    }

    private func buildPanel() {
        panel.translatesAutoresizingMaskIntoConstraints = false
        panel.backgroundColor = .systemBackground
        panel.layer.cornerRadius = 20
        panel.layer.cornerCurve = .continuous
        panel.layer.maskedCorners = [.layerMaxXMinYCorner, .layerMaxXMaxYCorner]
        panel.layer.masksToBounds = true
        addSubview(panel)
        panelLeading = panel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: -340)
        panelWidth = panel.widthAnchor.constraint(equalToConstant: 290)
        // Content-height card that emerges from the (vertically centered) handle:
        // its centre tracks the handle, clamped so it never leaves the screen, and
        // the activator list scrolls only if the card would ever overflow.
        let centerY = panel.centerYAnchor.constraint(equalTo: safeAreaLayoutGuide.centerYAnchor)
        centerY.priority = .defaultHigh
        let topClamp = panel.topAnchor.constraint(greaterThanOrEqualTo: safeAreaLayoutGuide.topAnchor, constant: 10)
        let bottomClamp = panel.bottomAnchor.constraint(lessThanOrEqualTo: safeAreaLayoutGuide.bottomAnchor, constant: -12)
        NSLayoutConstraint.activate([
            panelLeading, panelWidth, centerY, topClamp, bottomClamp,
        ])

        // Identity (top)
        iconView.translatesAutoresizingMaskIntoConstraints = false
        iconView.contentMode = .scaleAspectFill
        iconView.clipsToBounds = true
        iconView.layer.cornerRadius = 11
        iconView.layer.cornerCurve = .continuous
        nameLabel.font = .systemFont(ofSize: 17, weight: .semibold)
        nameLabel.textColor = .label
        versionLabel.font = .systemFont(ofSize: 12.5)
        versionLabel.textColor = .secondaryLabel
        let texts = UIStackView(arrangedSubviews: [nameLabel, versionLabel])
        texts.axis = .vertical; texts.spacing = 1
        let identity = UIStackView(arrangedSubviews: [iconView, texts])
        identity.axis = .horizontal; identity.spacing = 12; identity.alignment = .center
        identity.translatesAutoresizingMaskIntoConstraints = false
        panel.addSubview(identity)
        let topSep = makeSeparator(); panel.addSubview(topSep)

        // Activators (scroll, middle)
        actsStack.axis = .vertical; actsStack.spacing = 2
        actsStack.translatesAutoresizingMaskIntoConstraints = false
        let scroll = UIScrollView()
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.showsVerticalScrollIndicator = false
        scroll.addSubview(actsStack)
        panel.addSubview(scroll)

        // Utility (bottom)
        let botSep = makeSeparator(); panel.addSubview(botSep)
        let util = UIStackView(arrangedSubviews: [
            utilButton(image: bundledIcon("icon_settings"), systemName: "gearshape", title: "Settings") { [weak self] in self?.onHostEntry?(.settings); self?.close() },
            utilButton(image: bundledIcon("icon_download"), systemName: "arrow.down.circle", title: "Downloads") { [weak self] in self?.onHostEntry?(.downloads); self?.close() },
            utilButton(image: bundledIcon("icon_sidebar_collapse"), systemName: "sidebar.leading", title: "Close") { [weak self] in self?.close() },
        ])
        util.axis = .horizontal; util.distribution = .fillEqually
        util.translatesAutoresizingMaskIntoConstraints = false
        panel.addSubview(util)

        // Scroll wants to be exactly as tall as its content (so the card wraps);
        // it only scrolls if the activator list ever exceeds the screen cap.
        let scrollFit = scroll.heightAnchor.constraint(equalTo: actsStack.heightAnchor)
        scrollFit.priority = .defaultHigh
        NSLayoutConstraint.activate([
            iconView.widthAnchor.constraint(equalToConstant: 42),
            iconView.heightAnchor.constraint(equalToConstant: 42),
            identity.topAnchor.constraint(equalTo: panel.topAnchor, constant: 16),
            identity.leadingAnchor.constraint(equalTo: panel.leadingAnchor, constant: 18),
            identity.trailingAnchor.constraint(lessThanOrEqualTo: panel.trailingAnchor, constant: -16),

            topSep.topAnchor.constraint(equalTo: identity.bottomAnchor, constant: 14),
            topSep.leadingAnchor.constraint(equalTo: panel.leadingAnchor, constant: 14),
            topSep.trailingAnchor.constraint(equalTo: panel.trailingAnchor, constant: -14),

            scroll.topAnchor.constraint(equalTo: topSep.bottomAnchor, constant: 8),
            scroll.leadingAnchor.constraint(equalTo: panel.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: panel.trailingAnchor),
            scroll.bottomAnchor.constraint(equalTo: botSep.topAnchor, constant: -8),
            scrollFit,
            actsStack.topAnchor.constraint(equalTo: scroll.topAnchor),
            actsStack.leadingAnchor.constraint(equalTo: scroll.leadingAnchor),
            actsStack.trailingAnchor.constraint(equalTo: scroll.trailingAnchor),
            actsStack.bottomAnchor.constraint(equalTo: scroll.bottomAnchor),
            actsStack.widthAnchor.constraint(equalTo: scroll.widthAnchor),

            botSep.leadingAnchor.constraint(equalTo: panel.leadingAnchor, constant: 14),
            botSep.trailingAnchor.constraint(equalTo: panel.trailingAnchor, constant: -14),
            util.topAnchor.constraint(equalTo: botSep.bottomAnchor, constant: 8),
            util.leadingAnchor.constraint(equalTo: panel.leadingAnchor, constant: 10),
            util.trailingAnchor.constraint(equalTo: panel.trailingAnchor, constant: -10),
            util.bottomAnchor.constraint(equalTo: panel.bottomAnchor, constant: -12),
            util.heightAnchor.constraint(equalToConstant: 54),
        ])
    }

    // MARK: - Content

    private func reload() {
        // Identity: product name + version (app.json), icon from the launch lxapp.
        nameLabel.text = appConfig?.productName ?? "LingXia"
        versionLabel.text = (appConfig?.productVersion).map { "v\($0)" }
        // The identity is the product (host app), so use the host app's own icon —
        // not the home lxapp's icon (getLxAppInfo would give that).
        if let img = hostAppIcon() {
            iconView.image = img; iconView.backgroundColor = .clear
        } else {
            iconView.backgroundColor = .systemBlue
        }

        actsStack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        for activator in iosSidebarActivators() {
            actsStack.addArrangedSubview(activatorRow(activator))
        }
        actsStack.addArrangedSubview(browserRow())
    }

    /// Declared sidebar activators whose target surface is available on iOS.
    private func iosSidebarActivators() -> [LxAppUIConfig.Activator] {
        guard let ui = uiConfig else { return [] }
        return ui.activators
            .filter { $0.kind == .sidebarItem }
            .filter { surfaceAllowsIOS($0.action.surface) }
    }

    private func surfaceAllowsIOS(_ surfaceId: String) -> Bool {
        guard let surface = uiConfig?.surfaces.first(where: { $0.id == surfaceId }),
              let platforms = surface.platforms, !platforms.isEmpty else { return true }
        return platforms.contains("ios")
    }

    private func activatorRow(_ activator: LxAppUIConfig.Activator) -> UIView {
        let tap = ActivatorTap(id: activator.id, surface: activator.action.surface, action: activator.action.kind.rawValue)
        return iconRow(label: activator.label ?? activator.id,
                       image: resolvedIcon(activator.icon),
                       systemFallback: "square.dashed") { [weak self] in
            self?.onActivator?(tap); self?.close()
        }
    }

    private func browserRow() -> UIView {
        iconRow(label: "Browser", image: nil, systemFallback: "globe") { [weak self] in
            self?.onOpenBrowser?(); self?.close()
        }
    }

    // MARK: - Icons

    /// Resolve a declared icon path to a tintable image — PDFs (the build output)
    /// are rasterized through CoreGraphics; other formats load directly.
    private func resolvedIcon(_ path: String?) -> UIImage? {
        guard let path, !path.isEmpty,
              let url = uiConfigURL.flatMap({ LxAppAppUIBundleLoader.resolveRelativeResource(path, baseURL: $0) })
        else { return nil }
        if url.pathExtension.lowercased() == "pdf" { return pdfImage(url, height: 22) }
        return UIImage(contentsOfFile: url.path)?.withRenderingMode(.alwaysTemplate)
    }

    /// A bundled design icon (PDF) from the SDK's `icons/` resources, tintable.
    private func bundledIcon(_ name: String) -> UIImage? {
        #if SWIFT_PACKAGE
        let bundle = Bundle.module
        #else
        let bundle = Bundle(for: LxAppHostDrawer.self)
        #endif
        guard let url = bundle.url(forResource: name, withExtension: "pdf", subdirectory: "icons") else { return nil }
        return pdfImage(url, height: 20)
    }

    private func pdfImage(_ url: URL, height: CGFloat) -> UIImage? {
        guard let doc = CGPDFDocument(url as CFURL), let page = doc.page(at: 1) else { return nil }
        let box = page.getBoxRect(.cropBox)
        guard box.height > 0 else { return nil }
        let scale = height / box.height
        let size = CGSize(width: box.width * scale, height: box.height * scale)
        let img = UIGraphicsImageRenderer(size: size).image { ctx in
            let c = ctx.cgContext
            c.translateBy(x: 0, y: size.height)
            c.scaleBy(x: scale, y: -scale)
            c.drawPDFPage(page)
        }
        return img.withRenderingMode(.alwaysTemplate)
    }

    /// The host app's own icon, from the bundle's primary AppIcon set.
    private func hostAppIcon() -> UIImage? {
        guard let icons = Bundle.main.infoDictionary?["CFBundleIcons"] as? [String: Any],
              let primary = icons["CFBundlePrimaryIcon"] as? [String: Any],
              let files = primary["CFBundleIconFiles"] as? [String],
              let name = files.last else { return nil }
        return UIImage(named: name)
    }

    // MARK: - Rows

    private func iconRow(label: String, image: UIImage?, systemFallback: String, action: @escaping () -> Void) -> UIView {
        let icon = UIImageView(image: image ?? UIImage(systemName: systemFallback, withConfiguration: UIImage.SymbolConfiguration(pointSize: 18, weight: .regular)))
        icon.translatesAutoresizingMaskIntoConstraints = false
        icon.contentMode = .scaleAspectFit
        icon.tintColor = .label
        icon.widthAnchor.constraint(equalToConstant: 24).isActive = true
        icon.heightAnchor.constraint(equalToConstant: 24).isActive = true

        let title = UILabel()
        title.text = label
        title.font = .systemFont(ofSize: 15)
        title.textColor = .label

        let row = UIStackView(arrangedSubviews: [icon, title, UIView()])
        row.axis = .horizontal; row.spacing = 13; row.alignment = .center
        row.isLayoutMarginsRelativeArrangement = true
        row.layoutMargins = UIEdgeInsets(top: 11, left: 14, bottom: 11, right: 14)
        row.translatesAutoresizingMaskIntoConstraints = false
        row.isUserInteractionEnabled = false

        let button = HighlightButton()
        button.translatesAutoresizingMaskIntoConstraints = false
        button.layer.cornerRadius = 10
        button.layer.cornerCurve = .continuous
        button.addSubview(row)
        NSLayoutConstraint.activate([
            row.topAnchor.constraint(equalTo: button.topAnchor),
            row.bottomAnchor.constraint(equalTo: button.bottomAnchor),
            row.leadingAnchor.constraint(equalTo: button.leadingAnchor),
            row.trailingAnchor.constraint(equalTo: button.trailingAnchor),
        ])
        button.addAction(UIAction { _ in action() }, for: .touchUpInside)

        let container = UIView()
        container.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(button)
        NSLayoutConstraint.activate([
            button.topAnchor.constraint(equalTo: container.topAnchor, constant: 1),
            button.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -1),
            button.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 8),
            button.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -8),
        ])
        return container
    }

    private func utilButton(image: UIImage? = nil, systemName: String, title: String, action: @escaping () -> Void) -> UIButton {
        var cfg = UIButton.Configuration.plain()
        cfg.image = image ?? UIImage(systemName: systemName, withConfiguration: UIImage.SymbolConfiguration(pointSize: 18, weight: .regular))
        cfg.title = title
        cfg.imagePlacement = .top
        cfg.imagePadding = 3
        cfg.baseForegroundColor = .secondaryLabel
        var t = AttributeContainer()
        t.font = .systemFont(ofSize: 11, weight: .medium)
        cfg.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { _ in t }
        let b = UIButton(configuration: cfg)
        b.addAction(UIAction { _ in action() }, for: .touchUpInside)
        return b
    }

    private func makeSeparator() -> UIView {
        let v = UIView()
        v.translatesAutoresizingMaskIntoConstraints = false
        v.backgroundColor = UIColor.label.withAlphaComponent(0.1)
        v.heightAnchor.constraint(equalToConstant: 1).isActive = true
        return v
    }

    @objc private func scrimTapped() { close() }
}

private final class HighlightButton: UIButton {
    override var isHighlighted: Bool {
        didSet {
            UIView.animate(withDuration: 0.12) { self.alpha = self.isHighlighted ? 0.55 : 1.0 }
        }
    }
}
#endif
