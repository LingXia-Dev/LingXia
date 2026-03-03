#if os(macOS)
import AppKit
import Combine
import CLingXiaRustAPI

/// Content-area top toolbar with back/home buttons and page title.
/// Subscribes to NavigationBarStateManager for live updates.
/// Collapses to zero height when the Rust state says show_navbar == false.
@MainActor
public class MacNavigationToolbar: NSView {

    struct Layout {
        static let height: CGFloat = 38
        static let buttonSize: CGFloat = 28
        static let leadingPadding: CGFloat = 12
        static let titleLeading: CGFloat = 8
    }

    private let contentContainer = NSView()
    private let backButton = NSButton()
    private let homeButton = NSButton()
    private let titleLabel = NSTextField(labelWithString: "")
    private let separator = NSView()
    private var heightConstraint: NSLayoutConstraint!
    private var cancellables = Set<AnyCancellable>()

    private var showNavbar = false
    private var forceHidden = false

    /// Called with "back" or "home" when user clicks a nav button
    var onNavigationAction: ((String) -> Void)?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setupViews()
        subscribeToState()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
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
        backButton.image = NSImage(systemSymbolName: "chevron.left", accessibilityDescription: "Back")
        backButton.isBordered = false
        backButton.bezelStyle = .regularSquare
        backButton.imagePosition = .imageOnly
        backButton.contentTintColor = NSColor.labelColor.withAlphaComponent(0.8)
        backButton.target = self
        backButton.action = #selector(backClicked)
        backButton.isHidden = true
        contentContainer.addSubview(backButton)

        // Home button
        homeButton.translatesAutoresizingMaskIntoConstraints = false
        homeButton.image = NSImage(systemSymbolName: "house", accessibilityDescription: "Home")
        homeButton.isBordered = false
        homeButton.bezelStyle = .regularSquare
        homeButton.imagePosition = .imageOnly
        homeButton.contentTintColor = NSColor.labelColor.withAlphaComponent(0.8)
        homeButton.target = self
        homeButton.action = #selector(homeClicked)
        homeButton.isHidden = true
        contentContainer.addSubview(homeButton)

        // Title
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.font = NSFont.systemFont(ofSize: 13, weight: .medium)
        titleLabel.textColor = NSColor.labelColor
        titleLabel.lineBreakMode = .byTruncatingTail
        titleLabel.maximumNumberOfLines = 1
        contentContainer.addSubview(titleLabel)

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

            titleLabel.leadingAnchor.constraint(equalTo: backButton.trailingAnchor, constant: Layout.titleLeading),
            titleLabel.centerYAnchor.constraint(equalTo: contentContainer.centerYAnchor),
            titleLabel.trailingAnchor.constraint(lessThanOrEqualTo: contentContainer.trailingAnchor, constant: -12),
        ])

        // Bottom separator
        separator.translatesAutoresizingMaskIntoConstraints = false
        separator.wantsLayer = true
        separator.layer?.backgroundColor = NSColor.separatorColor.cgColor
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
        showNavbar = state?.show_navbar ?? false
        updateHeight()

        guard let state = state, showNavbar else {
            backButton.isHidden = true
            homeButton.isHidden = true
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

        titleLabel.stringValue = state.title_text.toString()

        // Foreground color from text_style (same logic as iOS NavigationBar)
        let textStyle = state.text_style.toString()
        let foregroundColor: NSColor = textStyle == "white" ? .white : .black
        backButton.contentTintColor = foregroundColor
        homeButton.contentTintColor = foregroundColor
        titleLabel.textColor = foregroundColor

        // Background color from state
        let bgColor = PlatformColor(argb: state.background_color)
        let alpha = CGFloat((state.background_color >> 24) & 0xFF) / 255.0
        if alpha > 0 {
            layer?.backgroundColor = bgColor.cgColor
        } else {
            layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
        }
    }

    /// Force-hide the toolbar (used when browser tab is active)
    func forceHide(_ hidden: Bool) {
        forceHidden = hidden
        updateHeight()
    }

    private func updateHeight() {
        let targetHeight: CGFloat = (showNavbar && !forceHidden) ? Layout.height : 0
        if heightConstraint.constant != targetHeight {
            heightConstraint.constant = targetHeight
        }
        separator.isHidden = !showNavbar || forceHidden
    }

    @objc private func backClicked() {
        onNavigationAction?("back")
    }

    @objc private func homeClicked() {
        onNavigationAction?("home")
    }
}

#endif
