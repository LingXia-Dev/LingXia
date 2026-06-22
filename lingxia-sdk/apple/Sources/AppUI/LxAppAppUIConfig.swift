import Foundation

struct LxAppGeneratedAppConfig: Decodable, Sendable {
    let productName: String
    let productVersion: String?
    let lingxiaId: String?
    let lingxiaServer: String?
    let homeAppId: String?
    let homeAppVersion: String?
    let cacheMaxSizeMB: Int?
}

struct LxAppUIConfig: Decodable, Sendable {
    let launch: Launch
    let surfaces: [Surface]
    let activators: [Activator]

    struct Launch: Decodable, Sendable {
        let initialSurface: String
        let openOnLaunch: Bool?
    }

    struct Surface: Decodable, Sendable {
        let id: String
        let role: Role
        let edge: Edge?
        let attachTo: String?
        let size: Size?
        let anchor: Anchor?
        let resizable: Bool?
        let showTrafficLights: Bool?
        let content: Content
        /// Availability filter. nil/empty = every platform; otherwise the concrete
        /// platforms it's available on (macos/windows/ios/android/harmony).
        let platforms: [String]?

        func isAvailable(on platform: String) -> Bool {
            guard let platforms, !platforms.isEmpty else { return true }
            return platforms.contains { $0.caseInsensitiveCompare(platform) == .orderedSame }
        }
    }

    enum Role: String, Decodable, Sendable {
        case main
        case aside
        case float
    }

    enum Edge: String, Decodable, Sendable {
        case left
        case right
        case top
        case bottom
    }

    enum Anchor: String, Decodable, Sendable {
        case activator
    }

    struct Size: Decodable, Sendable {
        let width: Double?
        let height: Double?
    }

    struct Content: Decodable, Sendable {
        let kind: Kind
        let appId: String?
        let path: String?
        let backend: String?

        enum Kind: String, Decodable, Sendable {
            case lxapp
            case terminal
        }
    }

    struct Activator: Decodable, Sendable {
        let id: String
        let kind: Kind
        let hostSurface: String?
        let label: String?
        let icon: String?
        let action: Action

        enum Kind: String, Decodable, Sendable {
            case menuBarItem
            case sidebarItem
            case toolbarItem
            case titlebarItem
            case appActivation
        }
    }

    struct Action: Decodable, Sendable {
        let kind: Kind
        let surface: String

        enum Kind: String, Decodable, Sendable {
            case toggleSurface
            case openSurface
        }
    }
}

struct LxAppGeneratedBundleConfig: Sendable {
    let app: LxAppGeneratedAppConfig
    let ui: LxAppUIConfig
    let appURL: URL
    let uiURL: URL
}

struct LxAppUIError: Error, LocalizedError, Sendable {
    let message: String

    var errorDescription: String? { message }

    static func missingResource(_ name: String) -> LxAppUIError {
        LxAppUIError(message: "Missing required bundle resource: \(name)")
    }

    static func invalidConfig(_ message: String) -> LxAppUIError {
        LxAppUIError(message: "Invalid app UI config: \(message)")
    }

    static func unsupported(_ message: String) -> LxAppUIError {
        LxAppUIError(message: "Unsupported app UI config: \(message)")
    }
}
