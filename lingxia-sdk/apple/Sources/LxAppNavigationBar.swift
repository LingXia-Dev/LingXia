import SwiftUI
import Foundation
import os.log

#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

struct NavigationButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed ? 0.95 : 1.0)
            .opacity(configuration.isPressed ? 0.7 : 1.0)
            .animation(.easeInOut(duration: 0.1), value: configuration.isPressed)
    }
}

@MainActor
public class NavigationBarStateManager: ObservableObject {
    @Published public var currentState: NavigationBarState? = nil
    public static let shared = NavigationBarStateManager()
    private init() {}

    public func updateState(appId: String, path: String) {
        let newState = LxPageNavigation.getNavigationBarState(appId: appId, path: path)
        currentState = newState
    }

    /// Force refresh state for a specific app
    public func refreshState(for appId: String) {
        #if os(iOS)
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first,
              let navController = window.rootViewController as? UINavigationController,
              let manager = navController.topViewController as? LxAppViewController,
              manager.currentAppId == appId,
              let path = manager.getCurrentPath() else { return }

        let newState = LxPageNavigation.getNavigationBarState(appId: appId, path: path)
        currentState = newState
        #endif
    }

    private func statesEqual(_ lhs: NavigationBarState?, _ rhs: NavigationBarState?) -> Bool {
        guard let lhs = lhs, let rhs = rhs else { return lhs == nil && rhs == nil }
        return lhs.show_navbar == rhs.show_navbar &&
               lhs.title_text.toString() == rhs.title_text.toString() &&
               lhs.background_color == rhs.background_color &&
               lhs.show_back_button == rhs.show_back_button &&
               lhs.show_home_button == rhs.show_home_button
    }
}

/// Extension to add helper methods to swift-bridge generated NavigationBarState
extension NavigationBarState {
    static let DEFAULT_HEIGHT: CGFloat = LxAppTheme.Metrics.navigationBarHeight
}

/// Clean data-driven navigation bar protocol
@MainActor
public protocol NavigationBarProtocol: AnyObject {
    /// Update UI based on NavigationBarState data (single source of truth)
    func updateWithState(_ state: NavigationBarState?)

    /// Get calculated height for layout purposes
    func getCalculatedContentHeight() -> CGFloat
}

/// Pure declarative SwiftUI Navigation Bar
/// Automatically renders based on NavigationBarState - no manual updates needed
public struct LxAppNavigationBarView: View {
    let state: NavigationBarState?
    let onBackTapped: () -> Void
    let onHomeTapped: () -> Void
    @State private var isLoading: Bool = false

    public init(
        state: NavigationBarState?,
        onBackTapped: @escaping () -> Void = {},
        onHomeTapped: @escaping () -> Void = {}
    ) {
        self.state = state
        self.onBackTapped = onBackTapped
        self.onHomeTapped = onHomeTapped
    }

    public var body: some View {
        if let state = state {
            let bgColor = state.show_navbar ? backgroundColor : Color.clear

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
                .frame(width: 52, alignment: .leading)

            // Center: Title
            Spacer()
            titleView
            Spacer()

            // Trailing: Space for capsule button
            Color.clear
                .frame(width: 52)
        }
        .frame(height: NavigationBarState.DEFAULT_HEIGHT)
    }

    @ViewBuilder
    private var leadingButton: some View {
        if let state = state, state.show_navbar {
            if state.show_back_button {
                Button(action: onBackTapped) {
                    LxAppIcons.back
                        .font(.system(size: 18, weight: .medium))
                        .foregroundColor(textColor)
                        .frame(width: 44, height: 44)
                        .contentShape(Rectangle())
                }
                .buttonStyle(NavigationButtonStyle())
            } else if state.show_home_button {
                Button(action: onHomeTapped) {
                    Image(systemName: "house")
                        .font(.system(size: 18, weight: .medium))
                        .foregroundColor(textColor)
                        .frame(width: 44, height: 44)
                        .contentShape(Rectangle())
                }
                .buttonStyle(NavigationButtonStyle())
            } else {
                Color.clear.frame(width: 44, height: 44)
            }
        } else {
            Color.clear.frame(width: 44, height: 44)
        }
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
public struct LxAppNavigationBarModifier: ViewModifier {
    let state: NavigationBarState?

    public init(state: NavigationBarState?) {
        self.state = state
    }

    public func body(content: Content) -> some View {
        VStack(spacing: 0) {
            if let state = state, state.show_navbar {
                LxAppNavigationBarView(state: state)
            }
            content
        }
    }
}

#if os(iOS)
import UIKit

@MainActor
public class iOSNavigationBarWrapper: UIView, NavigationBarProtocol {
    private var hostingController: UIHostingController<ReactiveNavigationBarView>?
    private var currentState: NavigationBarState?
    private var statusBarBackgroundView: UIView?
    public var heightConstraint: NSLayoutConstraint?

    public override init(frame: CGRect) {
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

    public func updateWithState(_ state: NavigationBarState?) {
        NavigationBarStateManager.shared.currentState = state

        let showNavbar = state?.show_navbar ?? false
        updateContainerHeight(showNavbar: showNavbar)

        if let state = state, showNavbar {
            let color = UIColor(argb: state.background_color)
            statusBarBackgroundView?.backgroundColor = color
            statusBarBackgroundView?.isHidden = false
        } else {
            statusBarBackgroundView?.backgroundColor = UIColor.clear
            statusBarBackgroundView?.isHidden = !showNavbar
        }
    }

    private func updateContainerHeight(showNavbar: Bool) {
        guard let heightConstraint = heightConstraint else { return }

        if showNavbar {
            // Show navbar: status bar + navbar content height
            let newHeight = LxAppTheme.getStatusBarHeight() + NavigationBarState.DEFAULT_HEIGHT
            heightConstraint.constant = newHeight
        } else {
            // CRITICAL FIX: In transparent mode, set height to 0 for complete transparency
            // This prevents any overlay that might cause black background
            heightConstraint.constant = 0
        }

        // Force layout update
        superview?.layoutIfNeeded()
    }

    public func getCalculatedContentHeight() -> CGFloat {
        return NavigationBarState.DEFAULT_HEIGHT
    }
}

struct ReactiveNavigationBarView: View {
    @ObservedObject private var stateManager = NavigationBarStateManager.shared

    var body: some View {
        LxAppNavigationBarView(
            state: stateManager.currentState,
            onBackTapped: handleBackTap,
            onHomeTapped: handleHomeTap
        )
    }

    private func handleBackTap() {
        print("🔙 Navigation back button tapped")
    }

    private func handleHomeTap() {
        print("🏠 Navigation home button tapped")
    }
}

public typealias LingXiaNavigationBar = iOSNavigationBarWrapper
public typealias PlatformNavigationBar = iOSNavigationBarWrapper
#elseif os(macOS)
public typealias LingXiaNavigationBar = LxAppNavigationBarView
public typealias PlatformNavigationBar = LxAppNavigationBarView
#endif

public extension View {
    /// Adds navigation bar to the view using clean data-driven state
    func lxAppNavigationBar(state: NavigationBarState?) -> some View {
        self.modifier(LxAppNavigationBarModifier(state: state))
    }
}
