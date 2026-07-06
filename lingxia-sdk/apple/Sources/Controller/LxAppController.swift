import Foundation
import OSLog

/// Manages the lifecycle of LxApp sessions: open, navigate, close.
///
/// Owns sessions, emits events via an `AsyncStream`, and delegates
/// decision points to interceptors.
///
/// ```swift
/// let controller = LxAppController()
/// for await event in controller.events {
///     switch event {
///     case .didOpen(let session): print(session.appId)
///     case .didClose(let session): print("closed \(session.appId)")
///     default: break
///     }
/// }
/// ```
@MainActor
public final class LxAppController {

    // MARK: - Properties

    /// Stable identifier for this controller instance.
    public let id = LxAppControllerID()

    /// Live sessions keyed by session ID.
    public private(set) var sessions: [LxAppSessionID: LxAppSession] = [:]

    // MARK: - Events

    /// Continuous stream of controller events. Multiple consumers are
    /// supported — each call to `events` returns an independent stream.
    public var events: AsyncStream<LxAppControllerEvent> {
        let id = nextContinuationId
        nextContinuationId += 1
        let (stream, continuation) = AsyncStream.makeStream(of: LxAppControllerEvent.self)
        continuations[id] = continuation
        continuation.onTermination = { [weak self] _ in
            Task { @MainActor in
                self?.continuations.removeValue(forKey: id)
            }
        }
        return stream
    }

    private var continuations: [Int: AsyncStream<LxAppControllerEvent>.Continuation] = [:]
    private var nextContinuationId = 0

    // MARK: - Interceptors

    private var interceptors: [LxAppControllerInterceptor:
        (LxAppInterceptContext) async -> LxAppInterceptDecision?] = [:]

    internal var hasInterceptors: Bool { !interceptors.isEmpty }

    /// Register an interceptor for a named decision point.
    ///
    /// The last registration wins. Pass `nil` to unregister.
    ///
    /// ```swift
    /// controller.setInterceptor(.willOpen) { ctx in
    ///     return .mountInHost(id: myHostView.id)
    /// }
    /// ```
    public func setInterceptor(
        _ kind: LxAppControllerInterceptor,
        handler: ((LxAppInterceptContext) async -> LxAppInterceptDecision?)?
    ) {
        interceptors[kind] = handler
    }

    // MARK: - Lifecycle

    private static let log = OSLog(subsystem: "LingXia", category: "LxAppController")

    public init() {}

    /// The currently tracked session for the given app ID, if any.
    public func session(forAppId appId: String) -> LxAppSession? {
        sessions.values.first { $0.appId == appId }
    }

    /// Open an LxApp from Swift code.
    ///
    @discardableResult
    public func open(_ request: LxAppOpenRequest) async throws -> LxAppSession {
        let sessionId = getLxAppSessionId(request.appId)
        guard sessionId > 0 else {
            let error = LxAppErrorPayload(
                code: "OPEN_REJECTED",
                message: "no session available for \(request.appId)",
                details: nil
            )
            emit(.didFailOpen(request: request, error: error))
            throw error
        }

        guard let session = await handleOpen(
            appId: request.appId,
            path: request.path,
            sessionId: sessionId,
            presentation: request.presentation,
            panelId: request.panelId ?? "",
            userInfo: request.userInfo
        ) else {
            throw LxAppErrorPayload(
                code: "OPEN_REJECTED",
                message: "open was rejected for \(request.appId)"
            )
        }
        return session
    }

    /// Synchronous open path used by the default product host runtime.
    ///
    /// This path intentionally rejects async interceptors. Advanced custom hosts
    /// should continue using the async `open(_:)` API.
    @discardableResult
    internal func openSync(_ request: LxAppOpenRequest) throws -> LxAppSession {
        let sessionId = getLxAppSessionId(request.appId)
        return try openSync(request, sessionId: sessionId)
    }

    /// Synchronous open path that preserves a caller-provided runtime session ID.
    ///
    /// This is used by FFI-driven runtime callbacks that already resolved the
    /// correct session on the Rust side and must not look it up again.
    @discardableResult
    internal func openSync(_ request: LxAppOpenRequest, sessionId: UInt64) throws -> LxAppSession {
        guard interceptors.isEmpty else {
            throw LxAppErrorPayload(
                code: "SYNC_OPEN_UNSUPPORTED",
                message: "openSync cannot be used when controller interceptors are installed"
            )
        }

        guard sessionId > 0 else {
            let error = LxAppErrorPayload(
                code: "OPEN_REJECTED",
                message: "no session available for \(request.appId)",
                details: nil
            )
            emit(.didFailOpen(request: request, error: error))
            throw error
        }

        emit(.willOpen(request))

        guard LxAppCore.executeOpenLxApp(
            appId: request.appId,
            path: request.path,
            sessionId: sessionId,
            presentation: request.presentation.ffiValue,
            panelId: request.panelId ?? ""
        ) else {
            let error = LxAppErrorPayload(
                code: "OPEN_REJECTED",
                message: "open was rejected for \(request.appId)"
            )
            emit(.didFailOpen(request: request, error: error))
            throw error
        }

        let session = LxAppSession(
            id: LxAppSessionID(rawValue: sessionId),
            appId: request.appId,
            path: request.path,
            presentation: request.presentation,
            userInfo: userInfoWithPageInstanceId(
                request.userInfo,
                appId: request.appId,
                path: request.path,
                sessionId: sessionId
            ),
            openedAt: Date()
        )
        sessions[session.id] = session
        emit(.didOpen(session))
        return session
    }

    /// Open the configured home app using this controller.
    @discardableResult
    public func openHomeApp(path: String = "") async throws -> LxAppSession {
        guard let homeAppId = LxAppRuntime.shared.info?.homeAppId ?? LxAppCore.getHomeLxAppId() else {
            throw LxAppErrorPayload(
                code: "HOME_APP_UNAVAILABLE",
                message: "No home app is available in the current runtime"
            )
        }
        return try await open(LxAppOpenRequest(appId: homeAppId, path: path))
    }

    // MARK: - Internal

    /// Handle an incoming open request with event emission.
    internal func handleOpen(
        appId: String,
        path: String,
        sessionId: UInt64,
        presentation: LxAppOpenPresentation = .normal,
        panelId: String = "",
        userInfo: [String: LxAppJSONValue] = [:]
    ) async -> LxAppSession? {
        let request = LxAppOpenRequest(
            appId: appId,
            path: path,
            presentation: presentation,
            panelId: panelId.isEmpty ? nil : panelId,
            userInfo: userInfo
        )
        emit(.willOpen(request))

        // Consult willOpen interceptor.
        let ctx = LxAppInterceptContext(
            controllerId: id,
            payload: encodeRequest(request)
        )
        var hostViewToMount: LxAppHostView?
        if let decision = await interceptors[.willOpen]?(ctx) {
            switch decision {
            case .handled:
                let session = LxAppSession(
                    id: LxAppSessionID(rawValue: sessionId),
                    appId: appId,
                    path: path,
                    presentation: presentation,
                    userInfo: request.userInfo,
                    openedAt: Date()
                )
                sessions[session.id] = session
                emit(.didOpen(session))
                return session
            case .reject(let reason):
                let error = LxAppErrorPayload(
                    code: "OPEN_REJECTED",
                    message: reason,
                    details: nil
                )
                emit(.didFailOpen(request: request, error: error))
                return nil
            case .mountInHost(let id):
                guard let hostView = LxAppHostView.resolve(id: id) else {
                    let error = LxAppErrorPayload(
                        code: "HOST_VIEW_UNAVAILABLE",
                        message: "host view \(id.rawValue.uuidString) is not registered"
                    )
                    emit(.didFailOpen(request: request, error: error))
                    return nil
                }
                hostViewToMount = hostView
            }
        }

        // Perform the platform open.
        guard LxAppCore.executeOpenLxApp(
            appId: appId,
            path: path,
            sessionId: sessionId,
            presentation: presentation.ffiValue,
            panelId: panelId
        ) else {
            let error = LxAppErrorPayload(
                code: "OPEN_REJECTED",
                message: "open was rejected for \(appId)"
            )
            emit(.didFailOpen(request: request, error: error))
            return nil
        }

        let session = LxAppSession(
            id: LxAppSessionID(rawValue: sessionId),
            appId: appId,
            path: path,
            presentation: presentation,
            userInfo: userInfoWithPageInstanceId(
                request.userInfo,
                appId: appId,
                path: path,
                sessionId: sessionId
            ),
            openedAt: Date()
        )
        sessions[session.id] = session
        if let hostViewToMount {
            do {
                try await hostViewToMount.mount(session)
            } catch {
                LXLog.error(
                    "mountInHost failed for \(appId) session=\(sessionId)",
                    category: "LxAppController",
                    error: error
                )
            }
        }
        emit(.didOpen(session))
        return session
    }

    /// Handle an incoming close request.
    @discardableResult
    internal func handleClose(appId: String, sessionId: UInt64) async -> Bool {
        let sid = LxAppSessionID(rawValue: sessionId)
        guard let session = sessions[sid], session.appId == appId else { return false }

        // Consult shouldClose interceptor.
        let ctx = LxAppInterceptContext(
            controllerId: id,
            payload: .object(["sessionId": .number(Double(sessionId))])
        )
        if let decision = await interceptors[.shouldClose]?(ctx) {
            if case .reject = decision { return false }
        }

        sessions.removeValue(forKey: sid)
        emit(.didClose(session))
        return true
    }

    // MARK: - Public: Swift-initiated actions

    /// Navigate within a session.
    public func navigate(_ request: LxAppNavigateRequest) {
        guard var session = sessions[request.sessionId] else {
            LXLog.error("navigate: unknown session \(String(describing: request.sessionId))", category: "LxAppController")
            return
        }

        LxAppCore.executeNavigation(
            appId: session.appId,
            path: request.path,
            animationType: request.animation
        )

        session.path = request.path
        session.userInfo = userInfoWithPageInstanceId(
            session.userInfo,
            appId: session.appId,
            path: request.path,
            sessionId: request.sessionId.rawValue
        )
        sessions[request.sessionId] = session
        emit(.didNavigate(sessionId: request.sessionId, to: request.path))
    }

    /// Close a session from the Swift side.
    ///
    /// Consults `shouldClose` interceptor. Returns `true` if closed,
    /// `false` if the interceptor rejected.
    @discardableResult
    public func close(_ sessionId: LxAppSessionID) async -> Bool {
        guard let session = sessions[sessionId] else { return false }

        // Consult shouldClose interceptor.
        let ctx = LxAppInterceptContext(
            controllerId: id,
            payload: .object(["sessionId": .number(Double(sessionId.rawValue))])
        )
        if let decision = await interceptors[.shouldClose]?(ctx) {
            if case .reject = decision { return false }
        }

        // Perform the platform close.
        LxAppCore.executeCloseLxApp(appId: session.appId, sessionId: sessionId.rawValue)

        sessions.removeValue(forKey: sessionId)
        emit(.didClose(session))
        return true
    }

    // MARK: - Private

    @discardableResult
    @_spi(Runner) public func discardSession(appId: String, sessionId: UInt64) -> LxAppSession? {
        let sid = LxAppSessionID(rawValue: sessionId)
        guard let session = sessions[sid], session.appId == appId else { return nil }
        sessions.removeValue(forKey: sid)
        return session
    }

    private func emit(_ event: LxAppControllerEvent) {
        for (_, c) in continuations {
            c.yield(event)
        }
    }

    private func userInfoWithPageInstanceId(
        _ userInfo: [String: LxAppJSONValue],
        appId: String,
        path: String,
        sessionId: UInt64
    ) -> [String: LxAppJSONValue] {
        var result = userInfo
        if let pageInstanceId = WebViewManager.resolvePageInstanceId(
            appId: appId,
            path: path,
            sessionId: sessionId
        ) {
            result["pageInstanceId"] = .string(pageInstanceId)
        } else {
            result.removeValue(forKey: "pageInstanceId")
        }
        return result
    }

    private func encodeRequest(_ request: LxAppOpenRequest) -> LxAppJSONValue {
        var dict: [String: LxAppJSONValue] = [
            "appId": .string(request.appId),
            "path": .string(request.path),
            "presentation": .string(request.presentation.rawValue),
        ]
        if let panelId = request.panelId {
            dict["panelId"] = .string(panelId)
        }
        if !request.userInfo.isEmpty {
            dict["userInfo"] = .object(request.userInfo)
        }
        return .object(dict)
    }
}

// MARK: - Presentation helpers

private extension LxAppOpenPresentation {
    var ffiValue: Int32 {
        switch self {
        case .normal: return 0
        case .panel:  return 1
        }
    }
}
