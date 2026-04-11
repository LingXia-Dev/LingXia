import SwiftUI

/// SwiftUI wrapper around `LxAppHostView`.
///
/// Since `LxAppHostView` uses `attach(_:)` to bind a WKWebView,
/// the representable is a lightweight shell. The caller is responsible
/// for providing the host view instance.
#if os(macOS)
public struct LxAppHostViewRepresentable: NSViewRepresentable {
    public let hostView: LxAppHostView
    public var onEvent: ((LxAppHostViewEvent) -> Void)?

    public init(hostView: LxAppHostView, onEvent: ((LxAppHostViewEvent) -> Void)? = nil) {
        self.hostView = hostView
        self.onEvent = onEvent
    }

    public init(controller: LxAppController, onEvent: ((LxAppHostViewEvent) -> Void)? = nil) {
        self.init(hostView: LxAppHostView(controller: controller), onEvent: onEvent)
    }

    public func makeNSView(context: Context) -> LxAppHostView {
        if let onEvent {
            context.coordinator.observe(view: hostView, handler: onEvent)
        }
        return hostView
    }

    public func updateNSView(_ view: LxAppHostView, context: Context) {}

    public func makeCoordinator() -> Coordinator { Coordinator() }
}

#elseif os(iOS)
public struct LxAppHostViewRepresentable: UIViewRepresentable {
    public let hostView: LxAppHostView
    public var onEvent: ((LxAppHostViewEvent) -> Void)?

    public init(hostView: LxAppHostView, onEvent: ((LxAppHostViewEvent) -> Void)? = nil) {
        self.hostView = hostView
        self.onEvent = onEvent
    }

    public init(controller: LxAppController, onEvent: ((LxAppHostViewEvent) -> Void)? = nil) {
        self.init(hostView: LxAppHostView(controller: controller), onEvent: onEvent)
    }

    public func makeUIView(context: Context) -> LxAppHostView {
        if let onEvent {
            context.coordinator.observe(view: hostView, handler: onEvent)
        }
        return hostView
    }

    public func updateUIView(_ view: LxAppHostView, context: Context) {}

    public func makeCoordinator() -> Coordinator { Coordinator() }
}
#endif

// MARK: - Shared Coordinator

extension LxAppHostViewRepresentable {
    @MainActor
    public final class Coordinator {
        private var task: Task<Void, Never>?

        func observe(view: LxAppHostView, handler: @escaping (LxAppHostViewEvent) -> Void) {
            task?.cancel()
            task = Task {
                for await event in view.events {
                    handler(event)
                }
            }
        }

        deinit {
            task?.cancel()
        }
    }
}

// MARK: - Modifier convenience

extension LxAppHostViewRepresentable {
    /// Attach an event handler.
    public func onEvent(_ handler: @escaping (LxAppHostViewEvent) -> Void) -> Self {
        var copy = self
        copy.onEvent = handler
        return copy
    }
}
