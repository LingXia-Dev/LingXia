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
        if !statesEqual(currentState, newState) {
            currentState = newState
        }
    }

    /// Force refresh state for a specific app
    public func refreshState(for appId: String) {
        #if os(iOS)
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first,
              let navController = window.rootViewController as? UINavigationController,
              let currentVC = navController.topViewController as? iOSLxAppViewController,
              currentVC.appId == appId else { return }

        let newState = LxPageNavigation.getNavigationBarState(appId: appId, path: currentVC.currentPath)
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
        // UI automatically reflects state
        Group {
            if let state = state, state.show_navbar {
                VStack(spacing: 0) {
                    // Status bar area with background color
                    Rectangle()
                        .fill(backgroundColor)
                        .frame(height: LxAppTheme.Metrics.statusBarHeight)

                    // Navigation bar content
                    navigationBarContent
                        .frame(height: NavigationBarState.DEFAULT_HEIGHT)
                }
                .background(backgroundColor)
            } else {
                // Hidden navbar: transparent status bar area
                Rectangle()
                    .fill(Color.clear)
                    .frame(height: LxAppTheme.Metrics.statusBarHeight)
            }
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
        if let state = state {
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
        if isLoading {
            ProgressView()
                .progressViewStyle(CircularProgressViewStyle())
                .scaleEffect(0.8)
        } else if let state = state {
            Text(state.title_text.toString())
                .font(LxAppTheme.Typography.navigationTitle)
                .foregroundColor(textColor)
                .lineLimit(1)
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
    public var heightConstraint: NSLayoutConstraint?

    public override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = UIColor.clear
        setupReactiveView()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        backgroundColor = UIColor.clear
        setupReactiveView()
    }

    /// 🎯 REACTIVE: Setup SwiftUI view that automatically responds to state changes
    private func setupReactiveView() {
        let reactiveView = ReactiveNavigationBarView()
        let hostingController = UIHostingController(rootView: reactiveView)
        hostingController.view.backgroundColor = UIColor.clear
        hostingController.view.translatesAutoresizingMaskIntoConstraints = false

        addSubview(hostingController.view)
        NSLayoutConstraint.activate([
            hostingController.view.topAnchor.constraint(equalTo: topAnchor),
            hostingController.view.leadingAnchor.constraint(equalTo: leadingAnchor),
            hostingController.view.trailingAnchor.constraint(equalTo: trailingAnchor),
            hostingController.view.bottomAnchor.constraint(equalTo: bottomAnchor)
        ])

        self.hostingController = hostingController
    }

    public func updateWithState(_ state: NavigationBarState?) {
        NavigationBarStateManager.shared.currentState = state
        self.isHidden = !(state?.show_navbar ?? false)

        // Update UIKit container height based on navbar visibility
        updateContainerHeight(showNavbar: state?.show_navbar ?? false)
    }

    private func updateContainerHeight(showNavbar: Bool) {
        guard let heightConstraint = heightConstraint else { return }

        let statusBarHeight = window?.windowScene?.statusBarManager?.statusBarFrame.height ?? LxAppTheme.Metrics.statusBarHeight

        if showNavbar {
            // Show navbar: status bar + navbar height
            heightConstraint.constant = statusBarHeight + NavigationBarState.DEFAULT_HEIGHT
        } else {
            // Hide navbar: only status bar height
            heightConstraint.constant = statusBarHeight
        }

        // Force layout update
        setNeedsLayout()
        layoutIfNeeded()
    }

    public func getCalculatedContentHeight() -> CGFloat {
        return NavigationBarState.DEFAULT_HEIGHT
    }
}

struct ReactiveNavigationBarView: View {
    @StateObject private var stateManager = NavigationBarStateManager.shared

    var body: some View {
        LxAppNavigationBarView(
            state: stateManager.currentState,
            onBackTapped: handleBackTap,
            onHomeTapped: handleHomeTap
        )
    }

    private func handleBackTap() {
    }

    private func handleHomeTap() {
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
