import SwiftUI
import Foundation

#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

@MainActor
final class NavigationBarStateManager: ObservableObject {
    @Published var currentState: NavigationBarState? = nil
    @Published var currentAppId: String? = nil
    static let shared = NavigationBarStateManager()

    private init() {}

    func updateState(appId: String, path: String) {
        guard !appId.isEmpty, !path.isEmpty else {
            currentState = nil
            return
        }
        let newState = lingxia.getNavigationBarState(appId, path)
        currentState = newState
        currentAppId = appId
    }

    /// Force refresh state for a specific app
    func refreshState(for appId: String) {
        #if os(iOS)
        // Get the current LxAppViewController from iOSLxApp
        guard let lxAppManager = iOSLxApp.getInstance().currentLxAppManager else {
            return
        }

        let currentAppId = LxAppCore.currentAppId
        guard currentAppId == appId else {
            return
        }

        let path = lxAppManager.getCurrentPath()
        let newState = lingxia.getNavigationBarState(appId, path)
        currentState = newState

        // Force immediate UI update on main thread
        DispatchQueue.main.async {
            lxAppManager.updateNavigationBar(appId: appId, path: path)
            if let navigationBar = lxAppManager.globalNavigationBar {
                navigationBar.updateWithState(newState)
                navigationBar.setNeedsLayout()
                navigationBar.layoutIfNeeded()
            }
        }
        #elseif os(macOS)
        macOSLxApp.refreshNavigationBar(appId: appId)
        #endif
    }
}

/// Extension to add helper methods to swift-bridge generated NavigationBarState
extension NavigationBarState {
    static let DEFAULT_HEIGHT: CGFloat = LxAppTheme.Metrics.navigationBarHeight
}

/// Clean data-driven navigation bar protocol
@MainActor
protocol NavigationBarProtocol: AnyObject {
    /// Update UI based on NavigationBarState data (single source of truth)
    func updateWithState(_ state: NavigationBarState?)

    /// Get calculated height for layout purposes
    func getCalculatedContentHeight() -> CGFloat
}

/// Embedded navigation button for navbar
struct NavigationButton: View {
    let isBackButton: Bool
    let tintColor: Color
    let isEnabled: Bool
    let title: String?
    let action: () -> Void

    init(isBackButton: Bool, tintColor: Color = .primary, isEnabled: Bool = true, title: String? = nil, action: @escaping () -> Void) {
        self.isBackButton = isBackButton
        self.tintColor = tintColor
        self.isEnabled = isEnabled
        self.title = title
        self.action = action
    }

    var body: some View {
        Button(action: {
            if isEnabled {
                action()
            }
        }) {
            buttonContent
        }
        .frame(width: 44, height: 44)
        .buttonStyle(.plain)
        .opacity(isEnabled ? 1.0 : 0.4)
        .scaleEffect(isPressed && isEnabled ? 0.95 : 1.0)
        .animation(.easeInOut(duration: 0.1), value: isPressed)
        .onLongPressGesture(minimumDuration: 0, maximumDistance: .infinity, pressing: { pressing in
            if isEnabled {
                isPressed = pressing
            }
        }, perform: {})
        .background(Color.clear)
        .cornerRadius(8)
    }

    @ViewBuilder
    private var buttonContent: some View {
        if let title = title, !title.isEmpty {
            // Show title text for home button
            Text(title)
                .font(.system(size: 12, weight: .medium))
                .foregroundColor(foregroundColor)
                .lineLimit(1)
                .minimumScaleFactor(0.8)
        } else {
            // Show icon for back button
            #if os(iOS)
            if let uiImage = LxIcon.image(named: isBackButton ? "icon_back" : "icon_home", size: CGSize(width: 20, height: 20)) {
                Image(uiImage: uiImage)
                    .renderingMode(.template)
                    .foregroundColor(foregroundColor)
            }
            #else
            if let nsImage = LxIcon.image(named: isBackButton ? "icon_back" : "icon_home", size: CGSize(width: 20, height: 20)) {
                Image(nsImage: nsImage)
                    .renderingMode(.template)
                    .foregroundColor(foregroundColor)
            }
            #endif
        }
    }

    private var foregroundColor: Color {
        isEnabled ? tintColor : tintColor.opacity(0.5)
    }

    @State private var isPressed = false
}

/// Pure declarative SwiftUI Navigation Bar
/// Automatically renders based on NavigationBarState - no manual updates needed
struct macOSNavigationBarView: View {
    let state: NavigationBarState?
    let appId: String?
    let onBackTapped: () -> Void
    let onHomeTapped: () -> Void
    @State private var isLoading: Bool = false

    init(
        state: NavigationBarState?,
        appId: String? = nil,
        onBackTapped: @escaping () -> Void = {},
        onHomeTapped: @escaping () -> Void = {}
    ) {
        self.state = state
        self.appId = appId
        self.onBackTapped = onBackTapped
        self.onHomeTapped = onHomeTapped
    }

    /// Get the app name from LxAppInfo, returns nil if not available
    private var appName: String? {
        guard let appId = appId else { return nil }
        let info = getLxAppInfo(appId)
        let name = info.app_name.toString()
        return name.isEmpty ? nil : name
    }

    var body: some View {
        if let state = state {
            let bgColor = state.show_navbar ? backgroundColor : Color.clear

            // Main NavigationBar content - always show the structure
            VStack(spacing: 0) {
                Rectangle()
                    .fill(bgColor)
                    .frame(height: LxAppTheme.getStatusBarHeight())

                navigationBarContent
                    .frame(height: NavigationBarState.DEFAULT_HEIGHT)
                    .background(bgColor)
            }
            .background(bgColor)
            .ignoresSafeArea(.container, edges: .top)
            .clipped()
        }
    }

    private var navigationBarContent: some View {
        HStack(alignment: .center, spacing: 0) {
            // Leading: Back/Home button
            leadingButton
                .padding(.leading, 4)

            // Center: Title
            Spacer()
            titleView
            Spacer()

            // Trailing: Space for capsule button (iOS only, macOS has no leading button to balance)
            #if os(iOS)
            Color.clear
                .frame(width: 44 + 10) // Match the leading button's effective width (44 button + 10 padding)
            #endif
        }
        .frame(height: NavigationBarState.DEFAULT_HEIGHT)
    }

    @ViewBuilder
    private var leadingButton: some View {
        #if os(iOS)
        // Fixed position container - always reserves space for the button
        ZStack {
            if let state = state, state.show_navbar {
                if state.show_back_button {
                    // Back button is enabled - show back icon
                    NavigationButton(isBackButton: true, tintColor: textColor, isEnabled: true, action: onBackTapped)
                } else if state.show_home_button {
                    // Home button is enabled - show home icon
                    NavigationButton(
                        isBackButton: false,
                        tintColor: textColor,
                        isEnabled: true,
                        action: onHomeTapped
                    )
                } else {
                    // No button needed but show disabled placeholder for consistent layout
                    NavigationButton(isBackButton: true, tintColor: textColor, isEnabled: false, action: {})
                }
            } else {
                // Navbar hidden or no state - show disabled placeholder
                NavigationButton(isBackButton: true, tintColor: textColor, isEnabled: false, action: {})
            }
        }
        .frame(width: 44, height: 44)
        #else
        // macOS: navigation buttons live in the tab bar, no leading space needed
        EmptyView()
        #endif
    }

    @ViewBuilder
    private var titleView: some View {
        if let state = state, state.show_navbar {
            if isLoading {
                ProgressView()
                    .progressViewStyle(CircularProgressViewStyle())
                    .scaleEffect(0.8)
            } else {
                Text(state.title_text.toString())
                    .font(LxAppTheme.Typography.navigationTitle)
                    .foregroundColor(textColor)
                    .lineLimit(1)
            }
        }
    }

    // Computed properties for clean data-driven rendering
    private var backgroundColor: Color {
        guard let state = state else { return Color.clear }
        let platformColor = PlatformColor(argb: state.background_color)
        return Color(platformColor)
    }

    private var textColor: Color {
        guard let state = state else { return Color.primary }
        let textStyle = state.text_style.toString()
        return textStyle == "white" ? Color.white : Color.black
    }
}

/// Clean data-driven ViewModifier for navigation bar
struct LxAppNavigationBarModifier: ViewModifier {
    let state: NavigationBarState?
    let appId: String?

    init(state: NavigationBarState?, appId: String? = nil) {
        self.state = state
        self.appId = appId
    }

    func body(content: Content) -> some View {
        VStack(spacing: 0) {
            if let state = state, state.show_navbar {
                macOSNavigationBarView(state: state, appId: appId)
            }
            content
        }
    }
}

#if os(iOS)
import UIKit

@MainActor
class iOSNavigationBarView: UIView, NavigationBarProtocol {
    private var hostingController: UIHostingController<ReactiveNavigationBarView>?
    private var currentState: NavigationBarState?
    private var statusBarBackgroundView: UIView?
    var heightConstraint: NSLayoutConstraint?

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = UIColor.clear
        clipsToBounds = false // Allow content to extend beyond bounds
        setupReactiveView()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        backgroundColor = UIColor.clear
        clipsToBounds = false // Allow content to extend beyond bounds
        setupReactiveView()
    }

    /// Setup SwiftUI view that automatically responds to state changes
    private func setupReactiveView() {
        let reactiveView = ReactiveNavigationBarView()
        let hostingController = UIHostingController(rootView: reactiveView)
        hostingController.view.backgroundColor = UIColor.clear
        hostingController.view.translatesAutoresizingMaskIntoConstraints = false
        hostingController.view.clipsToBounds = false // Allow SwiftUI content to extend beyond bounds

        addSubview(hostingController.view)
        NSLayoutConstraint.activate([
            hostingController.view.topAnchor.constraint(equalTo: topAnchor),
            hostingController.view.leadingAnchor.constraint(equalTo: leadingAnchor),
            hostingController.view.trailingAnchor.constraint(equalTo: trailingAnchor),
            hostingController.view.bottomAnchor.constraint(equalTo: bottomAnchor)
        ])

        self.hostingController = hostingController

        // Setup status bar background view using UIKit
        setupStatusBarBackground()
    }

    /// Setup a UIKit view for status bar background that we can control directly
    private func setupStatusBarBackground() {
        statusBarBackgroundView = UIView()
        statusBarBackgroundView?.translatesAutoresizingMaskIntoConstraints = false
        // CRITICAL: Set initial background to clear to prevent black flash
        statusBarBackgroundView?.backgroundColor = UIColor.clear

        if let statusBarBg = statusBarBackgroundView {
            addSubview(statusBarBg)
            NSLayoutConstraint.activate([
                statusBarBg.topAnchor.constraint(equalTo: topAnchor),
                statusBarBg.leadingAnchor.constraint(equalTo: leadingAnchor),
                statusBarBg.trailingAnchor.constraint(equalTo: trailingAnchor),
                statusBarBg.heightAnchor.constraint(equalToConstant: LxAppTheme.getStatusBarHeight())
            ])
        }
    }

    func updateWithState(_ state: NavigationBarState?) {
        NavigationBarStateManager.shared.currentState = state

        let showNavbar = state?.show_navbar ?? false
        updateContainerHeight(showNavbar: showNavbar)

        // Set the visibility of the entire NavigationBar
        self.isHidden = !showNavbar

        if let state = state, showNavbar {
            let color = UIColor(argb: state.background_color)
            statusBarBackgroundView?.backgroundColor = color
            statusBarBackgroundView?.isHidden = false
        } else {
            // Make navbar completely transparent
            statusBarBackgroundView?.backgroundColor = UIColor.clear
            statusBarBackgroundView?.isHidden = true
            self.backgroundColor = UIColor.clear
            self.layer.backgroundColor = UIColor.clear.cgColor
        }
    }

    private func updateContainerHeight(showNavbar: Bool) {
        guard let heightConstraint = heightConstraint else { return }

        let newHeight = LxAppTheme.getStatusBarHeight() + NavigationBarState.DEFAULT_HEIGHT
        if heightConstraint.constant != newHeight {
            heightConstraint.constant = newHeight
        }
    }

    func getCalculatedContentHeight() -> CGFloat {
        return NavigationBarState.DEFAULT_HEIGHT
    }
}

struct ReactiveNavigationBarView: View {
    @ObservedObject private var stateManager = NavigationBarStateManager.shared

    var body: some View {
        macOSNavigationBarView(
            state: stateManager.currentState,
            appId: stateManager.currentAppId,
            onBackTapped: handleBackTap,
            onHomeTapped: handleHomeTap
        )
    }

    private func handleBackTap() {
        // Get current app ID from state manager
        if let appId = stateManager.currentAppId {
            let _ = onLxappEvent(appId, LxAppEvent.navigationClick, LxAppEvent.navigationActionBack)
        }
    }

    private func handleHomeTap() {
        // Get current app ID from state manager
        if let appId = stateManager.currentAppId {
            let _ = onLxappEvent(appId, LxAppEvent.navigationClick, LxAppEvent.navigationActionHome)
        }
    }
}

typealias LingXiaNavigationBar = iOSNavigationBarView
#elseif os(macOS)
typealias LingXiaNavigationBar = macOSNavigationBarView
#endif

extension View {
    /// Adds navigation bar to the view using clean data-driven state
    func lxAppNavigationBar(state: NavigationBarState?, appId: String? = nil) -> some View {
        self.modifier(LxAppNavigationBarModifier(state: state, appId: appId))
    }
}
