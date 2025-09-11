import SwiftUI
import Foundation
import os.log

#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

@MainActor
public class NavigationBarStateManager: ObservableObject {
    @Published public var currentState: NavigationBarState? = nil
    @Published public var currentAppId: String? = nil
    public static let shared = NavigationBarStateManager()
    private init() {}

    public func updateState(appId: String, path: String) {
        currentState = LxPageNavigation.getNavigationBarState(appId: appId, path: path)
        currentAppId = appId
    }

    /// Force refresh state for a specific app
    public func refreshState(for appId: String) {
        #if os(iOS)
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first,
              let navController = window.rootViewController as? UINavigationController,
              let manager = navController.topViewController as? LxAppViewController,
              LxAppCore.currentAppId == appId else { return }
        let path = manager.getCurrentPath()
        let newState = LxPageNavigation.getNavigationBarState(appId: appId, path: path)
        currentState = newState
        #endif
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

/// Floating navigation button with glassmorphism effect
public struct NavigationButton: View {
    let isBackButton: Bool
    let action: () -> Void

    public var body: some View {
        Button(action: action) {
            ZStack {
                Circle()
                    .fill(backgroundMaterial)
                    .frame(width: 36, height: 36)
                    .shadow(color: .black.opacity(0.1), radius: 2, x: 0, y: 1)

                // Direct SwiftUI drawing - no image generation needed
                Canvas { context, size in
                    if isBackButton {
                        drawBackArrow(context: context, size: size)
                    } else {
                        drawHomeIcon(context: context, size: size)
                    }
                }
                .frame(width: 20, height: 20)
            }
        }
        .frame(width: 44, height: 44)
        .buttonStyle(.plain)
        .scaleEffect(isPressed ? 0.95 : 1.0)
        .animation(.easeInOut(duration: 0.1), value: isPressed)
        .onLongPressGesture(minimumDuration: 0, maximumDistance: .infinity, pressing: { pressing in
            isPressed = pressing
        }, perform: {})
    }

    // Platform-specific background material
    private var backgroundMaterial: some ShapeStyle {
        #if os(iOS)
        .ultraThinMaterial
        #else
        Color.black.opacity(0.2)
        #endif
    }

    @State private var isPressed = false

    private func drawBackArrow(context: GraphicsContext, size: CGSize) {
        let centerX = size.width / 2
        let centerY = size.height / 2
        let arrowSize: CGFloat = 12

        var path = Path()
        path.move(to: CGPoint(x: centerX + arrowSize/3, y: centerY - arrowSize/2))
        path.addLine(to: CGPoint(x: centerX - arrowSize/3, y: centerY))
        path.addLine(to: CGPoint(x: centerX + arrowSize/3, y: centerY + arrowSize/2))

        context.stroke(path, with: .color(.primary), style: StrokeStyle(lineWidth: 2.0, lineCap: .round, lineJoin: .round))
    }

    private func drawHomeIcon(context: GraphicsContext, size: CGSize) {
        let centerX = size.width / 2
        let centerY = size.height / 2
        let houseSize: CGFloat = 14

        // House roof
        var roofPath = Path()
        roofPath.move(to: CGPoint(x: centerX, y: centerY - houseSize/2))
        roofPath.addLine(to: CGPoint(x: centerX - houseSize/2, y: centerY))
        roofPath.addLine(to: CGPoint(x: centerX + houseSize/2, y: centerY))
        roofPath.closeSubpath()
        context.fill(roofPath, with: .color(.primary))

        // House base
        let baseRect = CGRect(x: centerX - houseSize/3, y: centerY, width: houseSize * 2/3, height: houseSize/2)
        context.fill(Path(baseRect), with: .color(.primary))
    }
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

            ZStack {
                // Main NavigationBar content
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

                // Floating button, shown only when navbar is hidden but a button is needed
                if !state.show_navbar {
                    VStack(spacing: 0) {
                        // This spacer pushes the content down, past the status bar area
                        Spacer()
                            .frame(height: LxAppTheme.getStatusBarHeight())

                        // This HStack holds the button and is the same size as the visible navbar's content area
                        HStack {
                            floatingNavigationButton
                                .padding(.leading, 10)
                            Spacer()
                        }
                        .frame(height: NavigationBarState.DEFAULT_HEIGHT)
                    }
                }
            }
        }
    }

    private var navigationBarContent: some View {
        HStack(alignment: .center, spacing: 0) {
            // Leading: Back/Home button
            leadingButton
                .padding(.leading, 10)

            // Center: Title
            Spacer()
            titleView
            Spacer()

            // Trailing: Space for capsule button
            Color.clear
                .frame(width: 44 + 10) // Match the leading button's effective width (44 button + 10 padding)
        }
        .frame(height: NavigationBarState.DEFAULT_HEIGHT)
    }

    @ViewBuilder
    private var leadingButton: some View {
        if let state = state, state.show_navbar {
            // Only show button in navbar when navbar is visible
            if state.show_back_button {
                NavigationButton(isBackButton: true, action: onBackTapped)
            } else if state.show_home_button {
                NavigationButton(isBackButton: false, action: onHomeTapped)
            } else {
                Color.clear.frame(width: 44, height: 44)
            }
        } else {
            Color.clear.frame(width: 44, height: 44)
        }
    }

    @ViewBuilder
    private var floatingNavigationButton: some View {
        if let state = state {
            if state.show_back_button {
                NavigationButton(isBackButton: true, action: onBackTapped)
            } else if state.show_home_button {
                NavigationButton(isBackButton: false, action: onHomeTapped)
            }
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
        // Get current app ID from state manager
        if let appId = stateManager.currentAppId {
            let _ = onUiEvent(appId, LxAppUIEvent.navigationClick, LxAppUIEvent.navigationActionBack)
        }
    }

    private func handleHomeTap() {
        // Get current app ID from state manager
        if let appId = stateManager.currentAppId {
            let _ = onUiEvent(appId, LxAppUIEvent.navigationClick, LxAppUIEvent.navigationActionHome)
        }
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
