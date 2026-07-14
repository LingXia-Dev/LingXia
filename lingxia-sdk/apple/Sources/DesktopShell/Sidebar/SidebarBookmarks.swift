#if os(macOS)
import AppKit
import CLingXiaRustAPI

/// Sidebar bookmarks model — decoded from `browserBookmarksSnapshotJson()`.
///
/// Three tiers: bookmarks are the archive (managed in the webui manager
/// page), pinned bookmarks are the high-frequency subset rendered as the
/// sidebar's top favicon grid (above lxapp tabs), and browser tabs are the
/// ephemeral session below. Invariant: pinned ⊆ bookmarked.
struct SidebarBookmarksSnapshot: Decodable {
    struct Group: Decodable {
        let id: String
        let name: String
    }

    struct Entry: Decodable {
        let id: String
        let url: String
        let title: String
        let groupId: String?
        let pinned: Bool?

        var isPinned: Bool { pinned ?? false }
    }

    let groups: [Group]
    let entries: [Entry]

    static let empty = SidebarBookmarksSnapshot(groups: [], entries: [])

    var pinnedEntries: [Entry] { entries.filter { $0.isPinned } }

    static func loadFromHost() -> SidebarBookmarksSnapshot {
        let json = browserBookmarksSnapshotJson().toString()
        guard !json.isEmpty, let data = json.data(using: .utf8) else { return .empty }
        return (try? JSONDecoder().decode(SidebarBookmarksSnapshot.self, from: data)) ?? .empty
    }

    /// Comparison key used to associate a pin with an existing browser tab.
    /// Thin wrapper over Rust's normalizer so the rules live in one place.
    static func normalize(_ raw: String) -> String {
        browserBookmarkNormalizeUrl(raw).toString()
    }
}

/// Native decoder for favicon files owned by Rust's cross-platform cache.
@MainActor
enum SidebarFaviconLoader {
    private static let cache = NSCache<NSString, NSImage>()

    static func originKey(for urlString: String) -> String? {
        guard let url = URL(string: urlString),
              let scheme = url.scheme?.lowercased(),
              scheme == "http" || scheme == "https",
              let host = url.host?.lowercased() else { return nil }
        var origin = URLComponents()
        origin.scheme = scheme
        origin.host = host
        origin.port = url.port
        return origin.url?.absoluteString
    }

    static func load(
        urlString: String,
        into apply: @escaping @MainActor @Sendable (NSImage) -> Void
    ) {
        let path = browserBookmarkFaviconPath(urlString).toString()
        guard !path.isEmpty else { return }
        let modified = (try? FileManager.default.attributesOfItem(atPath: path)[.modificationDate]
            as? Date)?.timeIntervalSince1970 ?? 0
        let cacheKey = "\(path)#\(modified)" as NSString
        if let cached = cache.object(forKey: cacheKey) {
            apply(cached)
            return
        }
        guard let image = NSImage(contentsOfFile: path), image.isValid else { return }
        cache.setObject(image, forKey: cacheKey)
        apply(image)
    }
}

/// One pin tile: a rounded favicon square in the sidebar's top grid.
/// Click opens the website or focuses its existing tab. Open state shows an
/// accent dot; focused state shows an accent ring. The normal tab stays visible.
@MainActor
final class SidebarPinTileView: NSView {

    enum Layout {
        static let size: CGFloat = 36
        static let cornerRadius: CGFloat = 9
        static let iconSize: CGFloat = 18
        static let gap: CGFloat = 5
        static let columns = 4
    }

    let bookmarkId: String
    private(set) var url: String
    private(set) var title: String

    var onOpen: ((String) -> Void)?
    /// An existing tab for this URL, when open (drives dot/ring + click).
    var openTabId: String?
    var onSelectTab: ((String) -> Void)?
    var onManageBookmarks: (() -> Void)?
    var onCloseTab: (() -> Void)?
    var onCloseOtherTabs: (() -> Void)?
    var onCloseTabsBelow: (() -> Void)?

    private let background = NSView()
    private let iconView = NSImageView()
    private let letterLabel = NSTextField(labelWithString: "")
    private let activeDot = NSView()
    private var trackingArea: NSTrackingArea?
    private var hovered = false

    var isFocused = false {
        didSet { refreshChrome() }
    }

    init(bookmarkId: String) {
        self.bookmarkId = bookmarkId
        self.url = ""
        self.title = ""
        super.init(frame: .zero)

        wantsLayer = true

        background.translatesAutoresizingMaskIntoConstraints = false
        background.wantsLayer = true
        background.layer?.cornerRadius = Layout.cornerRadius
        addSubview(background)

        iconView.translatesAutoresizingMaskIntoConstraints = false
        iconView.imageScaling = .scaleProportionallyDown
        addSubview(iconView)

        letterLabel.translatesAutoresizingMaskIntoConstraints = false
        letterLabel.font = .systemFont(ofSize: 14, weight: .bold)
        letterLabel.textColor = .secondaryLabelColor
        letterLabel.alignment = .center
        addSubview(letterLabel)

        activeDot.translatesAutoresizingMaskIntoConstraints = false
        activeDot.wantsLayer = true
        activeDot.layer?.cornerRadius = 2
        activeDot.layer?.backgroundColor = NSColor.controlAccentColor.cgColor
        activeDot.isHidden = true
        addSubview(activeDot)

        NSLayoutConstraint.activate([
            widthAnchor.constraint(equalToConstant: Layout.size),
            heightAnchor.constraint(equalToConstant: Layout.size),

            background.topAnchor.constraint(equalTo: topAnchor),
            background.bottomAnchor.constraint(equalTo: bottomAnchor),
            background.leadingAnchor.constraint(equalTo: leadingAnchor),
            background.trailingAnchor.constraint(equalTo: trailingAnchor),

            iconView.centerXAnchor.constraint(equalTo: centerXAnchor),
            iconView.centerYAnchor.constraint(equalTo: centerYAnchor),
            iconView.widthAnchor.constraint(equalToConstant: Layout.iconSize),
            iconView.heightAnchor.constraint(equalToConstant: Layout.iconSize),

            letterLabel.centerXAnchor.constraint(equalTo: centerXAnchor),
            letterLabel.centerYAnchor.constraint(equalTo: centerYAnchor),

            activeDot.centerXAnchor.constraint(equalTo: centerXAnchor),
            activeDot.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -4),
            activeDot.widthAnchor.constraint(equalToConstant: 4),
            activeDot.heightAnchor.constraint(equalToConstant: 4),
        ])
        refreshChrome()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func configure(url: String, title: String) {
        let changedOrigin = SidebarFaviconLoader.originKey(for: self.url)
            != SidebarFaviconLoader.originKey(for: url)
        self.url = url
        self.title = title
        let display = title.isEmpty ? (URL(string: url)?.host ?? url) : title
        toolTip = display
        let letter = String(display.prefix(1)).uppercased()
        letterLabel.stringValue = letter
        if changedOrigin { iconView.image = nil }
        if iconView.image == nil {
            letterLabel.isHidden = false
            SidebarFaviconLoader.load(urlString: url) { [weak self, requestedURL = url] image in
                guard let self, self.url == requestedURL else { return }
                self.iconView.image = image
                self.letterLabel.isHidden = true
            }
        }
    }

    private func refreshChrome() {
        let base: CGFloat = hovered ? 0.14 : 0.07
        background.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(base).cgColor
        background.layer?.borderWidth = isFocused ? 1.5 : 0
        background.layer?.borderColor = NSColor.controlAccentColor.cgColor
        activeDot.isHidden = openTabId == nil || isFocused
    }

    func syncState() {
        refreshChrome()
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let trackingArea { removeTrackingArea(trackingArea) }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.activeInKeyWindow, .mouseEnteredAndExited, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
        trackingArea = area
    }

    override func mouseEntered(with event: NSEvent) {
        hovered = true
        refreshChrome()
    }

    override func mouseExited(with event: NSEvent) {
        hovered = false
        refreshChrome()
    }

    override func mouseUp(with event: NSEvent) {
        guard bounds.contains(convert(event.locationInWindow, from: nil)) else { return }
        if let openTabId {
            onSelectTab?(openTabId)
        } else {
            onOpen?(url)
        }
    }

    override func menu(for event: NSEvent) -> NSMenu? {
        let menu = NSMenu()
        menu.autoenablesItems = false

        menu.addItem(tileItem("lx_browser_unpin", iconName: "icon_unpin", action: #selector(unpinClicked)))
        menu.addItem(.separator())
        menu.addItem(tileItem("lx_browser_copy_link", iconName: "icon_link", action: #selector(copyLinkClicked)))
        menu.addItem(tileItem(
            "lx_browser_open_in_system_browser", iconName: "icon_external", action: #selector(openExternalClicked)))
        menu.addItem(.separator())
        menu.addItem(tileItem(
            "lx_browser_remove_bookmark", iconName: "icon_bookmark_filled", action: #selector(removeClicked)))
        if onCloseTab != nil {
            menu.addItem(.separator())
            menu.addItem(tileItem("lx_common_close", iconName: "icon_close_x", action: #selector(closeTabClicked)))
            if onCloseOtherTabs != nil {
                menu.addItem(tileItem(
                    "lx_browser_close_other_tabs", iconName: "icon_close_other_tabs",
                    action: #selector(closeOtherTabsClicked)))
            }
            if onCloseTabsBelow != nil {
                menu.addItem(tileItem(
                    "lx_browser_close_tabs_below", iconName: "icon_close_tabs_below",
                    action: #selector(closeTabsBelowClicked)))
            }
        }
        menu.addItem(.separator())
        menu.addItem(tileItem(
            "lx_browser_manage_bookmarks", iconName: "icon_bookmarks", action: #selector(manageClicked)))

        return menu
    }

    private func tileItem(_ key: String, iconName: String, action: Selector) -> NSMenuItem {
        let item = NSMenuItem(title: L10n.string(key), action: action, keyEquivalent: "")
        item.image = LxIcon.image(named: iconName, size: CGSize(width: 16, height: 16))
        item.target = self
        return item
    }

    @objc private func unpinClicked() {
        _ = browserBookmarksCommand(
            #"{"op":"setPinned","id":"\#(jsonEscape(bookmarkId))","pinned":false}"#)
    }

    @objc private func copyLinkClicked() {
        // Anchor the toast to the visible viewport, not the (possibly tall)
        // scroll document view where it could render off-screen.
        BrowserPageMenu.copyLink(url, toastHost: enclosingScrollView?.contentView ?? superview)
    }

    @objc private func openExternalClicked() {
        guard let parsed = URL(string: url) else { return }
        NSWorkspace.shared.open(parsed)
    }

    @objc private func removeClicked() {
        _ = browserBookmarkRemoveByUrl(url)
    }

    @objc private func closeTabClicked() {
        onCloseTab?()
    }

    @objc private func closeOtherTabsClicked() {
        onCloseOtherTabs?()
    }

    @objc private func closeTabsBelowClicked() {
        onCloseTabsBelow?()
    }

    @objc private func manageClicked() {
        onManageBookmarks?()
    }
}

/// Escape a string for embedding inside a hand-built JSON literal.
@MainActor
func jsonEscape(_ s: String) -> String {
    var out = ""
    for c in s.unicodeScalars {
        switch c {
        case "\"": out += "\\\""
        case "\\": out += "\\\\"
        case "\n": out += "\\n"
        case "\r": out += "\\r"
        case "\t": out += "\\t"
        // Remaining control chars would produce invalid JSON if left raw.
        case let c where c.value < 0x20: out += String(format: "\\u%04x", c.value)
        default: out.unicodeScalars.append(c)
        }
    }
    return out
}

/// A pinned LXAPP tile in the sidebar's pin grid — the lxapp counterpart of
/// SidebarPinTileView (web pins). Click opens/focuses the lxapp as a MAIN;
/// the pin store itself lives in Rust (shellPinnedLxapps).
@MainActor
final class LxappPinTileView: NSView {
    let appId: String
    var onUnpin: (() -> Void)?

    private let background = NSView()
    private let iconView = NSImageView()

    init(appId: String) {
        self.appId = appId
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        wantsLayer = true

        background.translatesAutoresizingMaskIntoConstraints = false
        background.wantsLayer = true
        background.layer?.cornerRadius = SidebarPinTileView.Layout.cornerRadius
        background.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.06).cgColor
        addSubview(background)

        let info = getLxAppInfo(appId)
        let iconPath = info.icon.toString()
        let icon = (iconPath.isEmpty ? nil : NSImage(contentsOfFile: iconPath))
            ?? Bundle.lingxiaResources.url(
                forResource: "lxapp_default", withExtension: "png", subdirectory: "icons")
                .flatMap { NSImage(contentsOf: $0) }
        iconView.image = icon
        iconView.imageScaling = .scaleProportionallyDown
        iconView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(iconView)

        let name = info.app_name.toString()
        toolTip = name.isEmpty ? appId : name
        setAccessibilityElement(true)
        setAccessibilityRole(.button)
        setAccessibilityLabel(toolTip ?? appId)

        NSLayoutConstraint.activate([
            widthAnchor.constraint(equalToConstant: SidebarPinTileView.Layout.size),
            heightAnchor.constraint(equalToConstant: SidebarPinTileView.Layout.size),
            background.topAnchor.constraint(equalTo: topAnchor),
            background.leadingAnchor.constraint(equalTo: leadingAnchor),
            background.trailingAnchor.constraint(equalTo: trailingAnchor),
            background.bottomAnchor.constraint(equalTo: bottomAnchor),
            iconView.centerXAnchor.constraint(equalTo: centerXAnchor),
            iconView.centerYAnchor.constraint(equalTo: centerYAnchor),
            iconView.widthAnchor.constraint(equalToConstant: SidebarPinTileView.Layout.iconSize + 4),
            iconView.heightAnchor.constraint(equalToConstant: SidebarPinTileView.Layout.iconSize + 4),
        ])
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override func mouseDown(with event: NSEvent) {
        _ = shellOpenLxappMain(appId)
    }

    override func rightMouseDown(with event: NSEvent) {
        let menu = NSMenu()
        let unpin = NSMenuItem(
            title: L10n.string("lx_browser_unpin"),
            action: #selector(unpinClicked),
            keyEquivalent: ""
        )
        unpin.target = self
        menu.addItem(unpin)
        NSMenu.popUpContextMenu(menu, with: event, for: self)
    }

    @objc private func unpinClicked() {
        _ = shellSetLxappPinned(appId, false)
        onUnpin?()
    }

    override var mouseDownCanMoveWindow: Bool { false }
    override func accessibilityPerformPress() -> Bool {
        _ = shellOpenLxappMain(appId)
        return true
    }
}

#endif