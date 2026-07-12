#if os(macOS)
import AppKit
import CLingXiaRustAPI

/// Shared "page menu" for browser chrome (main toolbar ⋯ button, docked-aside
/// ⋯ button, and aside tab right-click). The menu header shows the page title
/// and URL — with no address bar in the aside, this header is the user's
/// anchor for "which site am I on", and Copy Link is the only way to export
/// the URL.
@MainActor
enum BrowserPageMenu {

    /// One page's menu context. Optional shell entries (bookmarks page) are
    /// only offered where a main-browser tab can host them.
    struct Context {
        let url: String
        let title: String
        /// View the toast anchors to (the web content area).
        weak var toastHost: NSView?
        /// Re-sync chrome (star button) after the bookmark state changed.
        var onBookmarkChanged: ((Bool) -> Void)?
        /// Open the bookmarks manager page (main browser only).
        var onOpenBookmarks: (() -> Void)?
        /// Open browser history (main browser only).
        var onOpenHistory: (() -> Void)?
        /// Open the current website's data-clearing dialog.
        var onClearSiteData: (() -> Void)?
    }

    static func menu(for context: Context) -> NSMenu {
        let menu = NSMenu()
        menu.autoenablesItems = false

        if let header = headerItem(title: context.title, url: context.url) {
            menu.addItem(header)
            menu.addItem(.separator())
        }

        let pageActionable = isPageActionable(context.url)

        let bookmarkActionable = isBookmarkActionable(context.url)
        let bookmarked = bookmarkActionable && browserBookmarkStatus(context.url)
        let bookmarkItem = actionItem(
            title: L10n.string(bookmarked ? "lx_browser_remove_bookmark" : "lx_browser_add_bookmark"),
            iconName: bookmarked ? "icon_bookmark_filled" : "icon_bookmark",
            key: "d",
            modifiers: [.command]
        ) { [url = context.url, title = context.title] in
            let nowBookmarked = browserBookmarkToggle(url, title)
            context.onBookmarkChanged?(nowBookmarked)
        }
        bookmarkItem.isEnabled = bookmarkActionable
        menu.addItem(bookmarkItem)

        let pinnedEntry = bookmarkActionable
            ? SidebarBookmarksSnapshot.loadFromHost().entries.first {
                SidebarBookmarksSnapshot.normalize($0.url)
                    == SidebarBookmarksSnapshot.normalize(context.url) && $0.isPinned
            }
            : nil
        let pinItem = actionItem(
            title: L10n.string(pinnedEntry == nil ? "lx_browser_pin_to_sidebar" : "lx_browser_unpin"),
            iconName: pinnedEntry == nil ? "icon_pin" : "icon_unpin"
        ) { [url = context.url, title = context.title, pinnedEntryId = pinnedEntry?.id] in
            if let pinnedEntryId {
                _ = browserBookmarksCommand(
                    #"{"op":"setPinned","id":"\#(jsonEscape(pinnedEntryId))","pinned":false}"#)
            } else {
                _ = browserBookmarkPin(url, title)
            }
            context.onBookmarkChanged?(browserBookmarkStatus(url))
        }
        pinItem.isEnabled = bookmarkActionable
        menu.addItem(pinItem)

        let copyItem = actionItem(
            title: L10n.string("lx_browser_copy_link"),
            iconName: "icon_link",
            key: "c",
            modifiers: [.command, .shift]
        ) { [url = context.url, weak host = context.toastHost] in
            copyLink(url, toastHost: host)
        }
        copyItem.isEnabled = pageActionable
        menu.addItem(copyItem)

        let externalItem = actionItem(
            title: L10n.string("lx_browser_open_in_system_browser"),
            iconName: "icon_external"
        ) { [url = context.url] in
            guard let parsed = URL(string: url) else { return }
            NSWorkspace.shared.open(parsed)
        }
        externalItem.isEnabled = pageActionable && !context.url.hasPrefix("lingxia://")
        menu.addItem(externalItem)

        if context.onOpenBookmarks != nil || context.onOpenHistory != nil
            || context.onClearSiteData != nil
        {
            menu.addItem(.separator())
            if let onOpenBookmarks = context.onOpenBookmarks {
                menu.addItem(actionItem(
                    title: L10n.string("lx_browser_manage_bookmarks"),
                    iconName: "icon_bookmarks",
                    handler: onOpenBookmarks
                ))
            }
            if let onOpenHistory = context.onOpenHistory {
                menu.addItem(actionItem(
                    title: L10n.string("lx_browser_history"),
                    iconName: "icon_history",
                    key: "y",
                    modifiers: [.command],
                    handler: onOpenHistory
                ))
            }
            if let onClearSiteData = context.onClearSiteData, isBookmarkActionable(context.url) {
                menu.addItem(.separator())
                menu.addItem(actionItem(
                    title: L10n.string("lx_browser_clear_site_data"),
                    iconName: "icon_clear_data",
                    handler: onClearSiteData
                ))
            }
        }

        return menu
    }

    /// A page qualifies for general URL actions when it is a real,
    /// user-visible location (startup pages and transient schemes are not).
    static func isPageActionable(_ url: String) -> Bool {
        let trimmed = url.trimmingCharacters(in: .whitespacesAndNewlines)
        return !trimmed.isEmpty && !browserUrlIsHidden(trimmed)
    }

    /// Only websites can be bookmarked or pinned. Built-in pages such as
    /// Settings and Downloads must remain normal sidebar entries.
    static func isBookmarkActionable(_ url: String) -> Bool {
        let trimmed = url.trimmingCharacters(in: .whitespacesAndNewlines)
        guard isPageActionable(trimmed),
              let scheme = URL(string: trimmed)?.scheme?.lowercased()
        else {
            return false
        }
        return scheme == "http" || scheme == "https"
    }

    static func copyLink(_ url: String, toastHost: NSView?) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(url, forType: .string)
        if let host = toastHost {
            BrowserToast.show(L10n.string("lx_browser_link_copied"), in: host)
        }
    }

    // MARK: - Items

    /// Disabled two-line header: page title over its URL.
    private static func headerItem(title: String, url: String) -> NSMenuItem? {
        let trimmedURL = url.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedURL.isEmpty, !browserUrlIsHidden(trimmedURL) else { return nil }
        let displayTitle = truncate(title.isEmpty ? hostOf(trimmedURL) : title, max: 44)
        let displayURL = truncateMiddle(trimmedURL, max: 52)

        let text = NSMutableAttributedString(
            string: displayTitle,
            attributes: [
                .font: NSFont.systemFont(ofSize: 12.5, weight: .semibold),
                .foregroundColor: NSColor.labelColor,
            ]
        )
        text.append(NSAttributedString(
            string: "\n" + displayURL,
            attributes: [
                .font: NSFont.systemFont(ofSize: 10.5),
                .foregroundColor: NSColor.secondaryLabelColor,
            ]
        ))
        let item = NSMenuItem()
        item.attributedTitle = text
        item.isEnabled = false
        return item
    }

    private static func actionItem(
        title: String,
        iconName: String,
        key: String = "",
        modifiers: NSEvent.ModifierFlags = [],
        handler: @escaping () -> Void
    ) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: #selector(MenuAction.fire(_:)), keyEquivalent: key)
        item.keyEquivalentModifierMask = modifiers
        item.image = LxIcon.image(named: iconName, size: CGSize(width: 16, height: 16))
        let proxy = MenuAction(handler)
        item.target = proxy
        // NSMenuItem does not retain its target; representedObject keeps the
        // proxy alive for the menu's lifetime.
        item.representedObject = proxy
        return item
    }

    private static func hostOf(_ url: String) -> String {
        URL(string: url)?.host ?? url
    }

    private static func truncate(_ s: String, max: Int) -> String {
        s.count <= max ? s : String(s.prefix(max - 1)) + "…"
    }

    private static func truncateMiddle(_ s: String, max: Int) -> String {
        guard s.count > max else { return s }
        let half = (max - 1) / 2
        return String(s.prefix(half)) + "…" + String(s.suffix(half))
    }

    @MainActor
    private final class MenuAction: NSObject {
        private let handler: () -> Void
        init(_ handler: @escaping () -> Void) { self.handler = handler }
        @objc func fire(_ sender: Any?) { handler() }
    }
}

/// Transient capsule feedback over web content ("Link copied").
@MainActor
enum BrowserToast {
    static func show(_ text: String, in host: NSView) {
        let label = NSTextField(labelWithString: text)
        label.font = .systemFont(ofSize: 12, weight: .semibold)
        label.textColor = .white
        label.alignment = .center

        let capsule = NSView()
        capsule.wantsLayer = true
        capsule.layer?.backgroundColor = NSColor.black.withAlphaComponent(0.78).cgColor
        capsule.layer?.cornerRadius = 14
        capsule.alphaValue = 0

        label.translatesAutoresizingMaskIntoConstraints = false
        capsule.translatesAutoresizingMaskIntoConstraints = false
        capsule.addSubview(label)
        host.addSubview(capsule)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: capsule.leadingAnchor, constant: 14),
            label.trailingAnchor.constraint(equalTo: capsule.trailingAnchor, constant: -14),
            label.topAnchor.constraint(equalTo: capsule.topAnchor, constant: 6),
            label.bottomAnchor.constraint(equalTo: capsule.bottomAnchor, constant: -6),
            capsule.centerXAnchor.constraint(equalTo: host.centerXAnchor),
            capsule.bottomAnchor.constraint(equalTo: host.bottomAnchor, constant: -28),
        ])

        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = 0.18
            capsule.animator().alphaValue = 1
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.6) { [weak capsule] in
            guard let capsule else { return }
            NSAnimationContext.runAnimationGroup({ ctx in
                ctx.duration = 0.25
                capsule.animator().alphaValue = 0
            }, completionHandler: {
                Task { @MainActor in
                    capsule.removeFromSuperview()
                }
            })
        }
    }
}
#endif
