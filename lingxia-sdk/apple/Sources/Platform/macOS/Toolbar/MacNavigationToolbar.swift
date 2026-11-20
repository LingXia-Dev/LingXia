#if os(macOS)
import AppKit
import Combine
import CLingXiaRustAPI

/// Content-area top toolbar with back/home/refresh buttons and page title.
/// Refresh follows Back (or Home), or takes the leading slot alone, on pages
/// that declared `enablePullDownRefresh`: a desktop has no pull gesture to
/// discover.
/// Subscribes to NavigationBarStateManager for live updates.
/// Collapses to zero height when the Rust state says show_navbar == false.
@MainActor
class MacNavigationToolbar: NSView {

    struct Layout {
        static let height: CGFloat = 38
        static let buttonSize: CGFloat = 28
        static let leadingPadding: CGFloat = 12
        static let titleLeading: CGFloat = 8
    }

    private let contentContainer = NSView()
    private let backButton = NSButton()
    private let homeButton = NSButton()
    private let refreshButton = NSButton()
    /// Refresh sits at the leading edge alone, or after the visible nav button.
    private var refreshAtLeading: NSLayoutConstraint!
    private var refreshAfterNav: NSLayoutConstraint!
    private let titleLabel = NSTextField(labelWithString: "")
    private let hostActionsStack = NSStackView()
    private let separator = NSView()
    private var heightConstraint: NSLayoutConstraint!
    private var cancellables = Set<AnyCancellable>()
    private var hostActionButtons: [NSButton] = []

    private var showNavbar = false
    private var forceHidden = false
    /// The window toolbar follows the shared (main) navigation state. An
    /// aside lxapp's bar is fed its own app's state through `apply(_:)`.
    private let followsSharedState: Bool
    private var ownedState: NavigationBarState?
    private var currentState: NavigationBarState? {
        followsSharedState ? NavigationBarStateManager.shared.currentState : ownedState
    }
    private var hasHostActions: Bool { !hostActionButtons.isEmpty }
    /// An aside's bar has nothing to add at the root of its stack: the slot
    /// tab already names the app. It shows only while Back or Home has
    /// somewhere to go, or the page offers Refresh.
    var collapsesWithoutNavigation = false

    /// Called with "back", "home" or "refresh" when user clicks a nav button
    var onNavigationAction: ((String) -> Void)?
    var onHostAction: ((String) -> Void)?

    override convenience init(frame frameRect: NSRect) {
        self.init(frame: frameRect, followsSharedState: true)
    }

    init(frame frameRect: NSRect = .zero, followsSharedState: Bool) {
        self.followsSharedState = followsSharedState
        super.init(frame: frameRect)
        setupViews()
        if followsSharedState {
            subscribeToState()
        }
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        separator.layer?.backgroundColor = LxAppHostTheme.separator.cgColor
        updateFromState(currentState)
    }

    private func setupViews() {
        wantsLayer = true
        clipsToBounds = true

        heightConstraint = heightAnchor.constraint(equalToConstant: 0)
        heightConstraint.isActive = true

        // Content container holds all children
        contentContainer.translatesAutoresizingMaskIntoConstraints = false
        addSubview(contentContainer)

        // Back button
        backButton.translatesAutoresizingMaskIntoConstraints = false
        backButton.image = LxIcon.image(named: "icon_back", size: CGSize(width: 16, height: 16))
        backButton.isBordered = false
        backButton.bezelStyle = .regularSquare
        backButton.imagePosition = .imageOnly
        backButton.contentTintColor = LxAppHostTheme.foreground.withAlphaComponent(0.8)
        backButton.target = self
        backButton.action = #selector(backClicked)
        backButton.setAccessibilityLabel("Back")
        backButton.isHidden = true
        contentContainer.addSubview(backButton)

        // Home button
        homeButton.translatesAutoresizingMaskIntoConstraints = false
        homeButton.image = LxIcon.image(named: "icon_home", size: CGSize(width: 16, height: 16))
        homeButton.isBordered = false
        homeButton.bezelStyle = .regularSquare
        homeButton.imagePosition = .imageOnly
        homeButton.contentTintColor = LxAppHostTheme.foreground.withAlphaComponent(0.8)
        homeButton.target = self
        homeButton.action = #selector(homeClicked)
        homeButton.setAccessibilityLabel("Home")
        homeButton.isHidden = true
        contentContainer.addSubview(homeButton)

        refreshButton.translatesAutoresizingMaskIntoConstraints = false
        refreshButton.image = LxIcon.image(named: "icon_browser_refresh", size: CGSize(width: 16, height: 16))
        refreshButton.isBordered = false
        refreshButton.bezelStyle = .regularSquare
        refreshButton.imagePosition = .imageOnly
        refreshButton.contentTintColor = LxAppHostTheme.foreground.withAlphaComponent(0.8)
        refreshButton.target = self
        refreshButton.action = #selector(refreshClicked)
        refreshButton.toolTip = L10n.string("lx_menu_refresh")
        refreshButton.setAccessibilityLabel(L10n.string("lx_menu_refresh"))
        refreshButton.isHidden = true
        contentContainer.addSubview(refreshButton)

        // Title (centered in full toolbar width)
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.font = NSFont.systemFont(ofSize: 13, weight: .medium)
        titleLabel.textColor = LxAppHostTheme.foreground
        titleLabel.alignment = .center
        titleLabel.lineBreakMode = .byTruncatingTail
        titleLabel.maximumNumberOfLines = 1
        titleLabel.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        contentContainer.addSubview(titleLabel)

        hostActionsStack.translatesAutoresizingMaskIntoConstraints = false
        hostActionsStack.orientation = .horizontal
        hostActionsStack.alignment = .centerY
        hostActionsStack.spacing = 6
        contentContainer.addSubview(hostActionsStack)

        NSLayoutConstraint.activate([
            contentContainer.topAnchor.constraint(equalTo: topAnchor),
            contentContainer.leadingAnchor.constraint(equalTo: leadingAnchor),
            contentContainer.trailingAnchor.constraint(equalTo: trailingAnchor),
            contentContainer.heightAnchor.constraint(equalToConstant: Layout.height),

            // Back button: at leading edge
            backButton.leadingAnchor.constraint(equalTo: contentContainer.leadingAnchor, constant: Layout.leadingPadding),
            backButton.centerYAnchor.constraint(equalTo: contentContainer.centerYAnchor),
            backButton.widthAnchor.constraint(equalToConstant: Layout.buttonSize),
            backButton.heightAnchor.constraint(equalToConstant: Layout.buttonSize),

            homeButton.leadingAnchor.constraint(equalTo: contentContainer.leadingAnchor, constant: Layout.leadingPadding),
            homeButton.centerYAnchor.constraint(equalTo: contentContainer.centerYAnchor),
            homeButton.widthAnchor.constraint(equalToConstant: Layout.buttonSize),
            homeButton.heightAnchor.constraint(equalToConstant: Layout.buttonSize),

            refreshButton.centerYAnchor.constraint(equalTo: contentContainer.centerYAnchor),
            refreshButton.widthAnchor.constraint(equalToConstant: Layout.buttonSize),
            refreshButton.heightAnchor.constraint(equalToConstant: Layout.buttonSize),

            titleLabel.centerXAnchor.constraint(equalTo: contentContainer.centerXAnchor),
            titleLabel.centerYAnchor.constraint(equalTo: contentContainer.centerYAnchor),
            titleLabel.leadingAnchor.constraint(greaterThanOrEqualTo: backButton.trailingAnchor, constant: Layout.titleLeading),
            titleLabel.leadingAnchor.constraint(greaterThanOrEqualTo: refreshButton.trailingAnchor, constant: Layout.titleLeading),
            titleLabel.trailingAnchor.constraint(lessThanOrEqualTo: hostActionsStack.leadingAnchor, constant: -12),

            hostActionsStack.trailingAnchor.constraint(equalTo: contentContainer.trailingAnchor, constant: -12),
            hostActionsStack.centerYAnchor.constraint(equalTo: contentContainer.centerYAnchor),
            hostActionsStack.leadingAnchor.constraint(greaterThanOrEqualTo: titleLabel.trailingAnchor, constant: 12),
        ])

        // Back and Home share the leading slot, so either one is the anchor.
        refreshAtLeading = refreshButton.leadingAnchor.constraint(
            equalTo: contentContainer.leadingAnchor, constant: Layout.leadingPadding)
        refreshAfterNav = refreshButton.leadingAnchor.constraint(
            equalTo: backButton.trailingAnchor, constant: 2)
        refreshAtLeading.isActive = true

        // Bottom separator
        separator.translatesAutoresizingMaskIntoConstraints = false
        separator.wantsLayer = true
        separator.layer?.backgroundColor = LxAppHostTheme.separator.cgColor
        addSubview(separator)

        NSLayoutConstraint.activate([
            separator.leadingAnchor.constraint(equalTo: leadingAnchor),
            separator.trailingAnchor.constraint(equalTo: trailingAnchor),
            separator.bottomAnchor.constraint(equalTo: bottomAnchor),
            separator.heightAnchor.constraint(equalToConstant: 1),
        ])
    }

    private func subscribeToState() {
        NavigationBarStateManager.shared.$currentState
            .receive(on: DispatchQueue.main)
            .sink { [weak self] state in
                self?.updateFromState(state)
            }
            .store(in: &cancellables)
    }

    private func updateFromState(_ state: NavigationBarState?) {
        showNavbar = state.map { state in
            state.show_navbar
                && (!collapsesWithoutNavigation || state.show_back_button || state.show_home_button
                    || state.show_refresh_button)
        } ?? false
        updateHeight()

        guard let state = state, showNavbar else {
            backButton.isHidden = true
            homeButton.isHidden = true
            refreshButton.isHidden = true
            titleLabel.stringValue = ""
            layer?.backgroundColor = nil
            return
        }

        // Follow Rust state exactly (same logic as iOS)
        let showBack = state.show_back_button
        let showHome = state.show_home_button && !showBack

        backButton.isHidden = !showBack
        backButton.isEnabled = showBack
        backButton.alphaValue = 1.0

        homeButton.isHidden = !showHome
        homeButton.isEnabled = showHome

        let showRefresh = state.show_refresh_button
        refreshButton.isHidden = !showRefresh
        refreshButton.isEnabled = showRefresh
        let followsNav = showBack || showHome
        if refreshAfterNav.isActive != followsNav {
            refreshAtLeading.isActive = false
            refreshAfterNav.isActive = false
            (followsNav ? refreshAfterNav : refreshAtLeading).isActive = true
        }

        titleLabel.stringValue = state.title_text.toString()

        let foregroundColor = PlatformColor(argb: state.foreground_color)
        backButton.contentTintColor = foregroundColor
        homeButton.contentTintColor = foregroundColor
        refreshButton.contentTintColor = foregroundColor
        titleLabel.textColor = foregroundColor
        separator.layer?.backgroundColor = PlatformColor(argb: state.divider_color).cgColor

        // Background color from state
        let bgColor = PlatformColor(argb: state.background_color)
        let alpha = CGFloat((state.background_color >> 24) & 0xFF) / 255.0
        if alpha > 0 {
            layer?.backgroundColor = bgColor.cgColor
        } else {
            layer?.backgroundColor = LxAppHostTheme.surfaceBackground.cgColor
        }

        needsLayout = true
        layoutSubtreeIfNeeded()
        superview?.needsLayout = true
        superview?.layoutSubtreeIfNeeded()
        window?.contentView?.needsLayout = true
        window?.contentView?.layoutSubtreeIfNeeded()
    }

    /// Force-hide the toolbar (used when browser tab is active)
    func forceHide(_ hidden: Bool) {
        forceHidden = hidden
        updateHeight()
    }

    func refreshCurrentState() {
        updateFromState(currentState)
    }

    /// Show `state` on a bar that does not follow the shared state.
    func apply(_ state: NavigationBarState?) {
        ownedState = state
        updateFromState(state)
    }

    func updateHostActions(_ items: [LxAppUIActionItem]) {
        hostActionButtons.forEach { button in
            hostActionsStack.removeArrangedSubview(button)
            button.removeFromSuperview()
        }
        hostActionButtons.removeAll()

        for item in items {
            let button = NSButton()
            button.translatesAutoresizingMaskIntoConstraints = false
            button.isBordered = false
            button.bezelStyle = .regularSquare
            button.imagePosition = .imageOnly
            button.toolTip = item.label
            button.identifier = NSUserInterfaceItemIdentifier(item.id)
            button.target = self
            button.action = #selector(hostActionClicked(_:))

            if let iconURL = item.iconURL, let image = NSImage(contentsOf: iconURL) {
                image.isTemplate = true
                button.image = image
                button.contentTintColor = LxAppHostTheme.mutedForeground
            } else {
                button.image = NSImage(systemSymbolName: "square.grid.2x2", accessibilityDescription: item.label)
                button.contentTintColor = LxAppHostTheme.mutedForeground
            }

            NSLayoutConstraint.activate([
                button.widthAnchor.constraint(equalToConstant: Layout.buttonSize),
                button.heightAnchor.constraint(equalToConstant: Layout.buttonSize),
            ])

            hostActionsStack.addArrangedSubview(button)
            hostActionButtons.append(button)
        }

        updateHeight()
    }

    private func updateHeight() {
        let targetHeight: CGFloat = ((showNavbar || hasHostActions) && !forceHidden) ? Layout.height : 0
        if heightConstraint.constant != targetHeight {
            heightConstraint.constant = targetHeight
        }
        separator.isHidden = forceHidden || (!showNavbar && !hasHostActions)
        needsLayout = true
        layoutSubtreeIfNeeded()
        superview?.needsLayout = true
        superview?.layoutSubtreeIfNeeded()
    }

    @objc private func backClicked() {
        onNavigationAction?("back")
    }

    @objc private func homeClicked() {
        onNavigationAction?("home")
    }

    @objc private func refreshClicked() {
        onNavigationAction?("refresh")
    }

    @objc private func hostActionClicked(_ sender: NSButton) {
        guard let actionID = sender.identifier?.rawValue else { return }
        onHostAction?(actionID)
    }
}

#endif
