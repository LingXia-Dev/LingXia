#if os(macOS)
import Foundation

struct SurfaceSwitcherSnapshot: Decodable {
    struct Icon: Decodable {
        let source: String
        let name: String?
        let uri: String?
        let provider: String?
        let key: String?
    }

    struct Item: Decodable {
        struct Content: Decodable {
            let kind: String
            let appId: String?
            let capability: String?
        }

        let surfaceId: String
        let content: Content
        let title: String?
        let icon: Icon?
        let active: Bool
        let root: Bool
        let closable: Bool
        let renameable: Bool
        let titleOverridden: Bool
    }

    let revision: UInt64
    let rootSurfaceId: String?
    let activeSurfaceId: String?
    let items: [Item]
}

@MainActor
enum SurfaceSwitcherBridge {
    private struct Registration: Encodable {
        let id: String
        let content: Content
        let presentation: Presentation
    }

    private enum Content: Encodable {
        case lxapp(appId: String, path: String?)
        case browser(initialUrl: String)
        case native(capability: String, instanceKey: String?)

        private enum CodingKeys: String, CodingKey {
            case kind, appId, path, initialUrl, capability, instanceKey
        }

        func encode(to encoder: Encoder) throws {
            var values = encoder.container(keyedBy: CodingKeys.self)
            switch self {
            case let .lxapp(appId, path):
                try values.encode("lxapp", forKey: .kind)
                try values.encode(appId, forKey: .appId)
                try values.encodeIfPresent(path, forKey: .path)
            case let .browser(initialUrl):
                try values.encode("browser", forKey: .kind)
                try values.encode(initialUrl, forKey: .initialUrl)
            case let .native(capability, instanceKey):
                try values.encode("native", forKey: .kind)
                try values.encode(capability, forKey: .capability)
                try values.encodeIfPresent(instanceKey, forKey: .instanceKey)
            }
        }
    }

    private enum Icon: Encodable {
        case builtIn(String)
        case providerAsset(provider: String, key: String)

        private enum CodingKeys: String, CodingKey {
            case source, name, provider, key
        }

        func encode(to encoder: Encoder) throws {
            var values = encoder.container(keyedBy: CodingKeys.self)
            switch self {
            case let .builtIn(name):
                try values.encode("builtIn", forKey: .source)
                try values.encode(name, forKey: .name)
            case let .providerAsset(provider, key):
                try values.encode("providerAsset", forKey: .source)
                try values.encode(provider, forKey: .provider)
                try values.encode(key, forKey: .key)
            }
        }
    }

    private struct Capabilities: Encodable {
        let close: Bool
        let rename: Bool
    }

    private struct Presentation: Encodable {
        let automaticTitle: String?
        let customTitle: String? = nil
        let icon: Icon?
        let capabilities: Capabilities
    }

    static func replaceDeclaredMains(
        ownerAppId: String,
        surfaces: [LxAppUIConfig.Surface],
        initialSurfaceID: String
    ) -> Bool {
        do {
            var mains = surfaces.filter { $0.role == .main }
            if let initialIndex = mains.firstIndex(where: { $0.id == initialSurfaceID }),
               initialIndex != mains.startIndex {
                mains.insert(mains.remove(at: initialIndex), at: mains.startIndex)
            }
            let registrations = try mains.map(makeRegistration)
            let data = try JSONEncoder().encode(registrations)
            guard let json = String(data: data, encoding: .utf8) else { return false }
            return replaceHostMains(ownerAppId, json)
        } catch {
            LXLog.error(
                "Failed to encode main surface registrations",
                category: "SurfaceSwitcher",
                error: error
            )
            return false
        }
    }

    static func registerDeclaredNativeAsides(
        ownerAppId: String,
        surfaces: [LxAppUIConfig.Surface]
    ) -> Bool {
        for surface in surfaces where surface.role == .aside && surface.content.kind == .native {
            guard let capability = surface.content.name?.rawValue,
                  registerHostNativeAsideDeclaration(
                      ownerAppId,
                      surface.id,
                      capability,
                      surface.edge?.rawValue ?? "right"
                  )
            else { return false }
        }
        return true
    }

    static func openDeclaredMain(
        ownerAppId: String,
        surface: LxAppUIConfig.Surface
    ) -> Bool {
        do {
            let data = try JSONEncoder().encode(makeRegistration(surface))
            guard let json = String(data: data, encoding: .utf8) else { return false }
            return openHostMain(ownerAppId, json)
        } catch {
            LXLog.error(
                "Failed to encode main surface registration",
                category: "SurfaceSwitcher",
                error: error
            )
            return false
        }
    }

    static func snapshot(ownerAppId: String) -> SurfaceSwitcherSnapshot? {
        let json = surfaceSwitcher(ownerAppId).toString()
        guard let data = json.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(SurfaceSwitcherSnapshot.self, from: data)
    }

    static func resolvedIcon(
        _ icon: SurfaceSwitcherSnapshot.Icon?
    ) -> (url: URL?, builtIn: String?) {
        guard let icon else { return (nil, nil) }
        switch icon.source {
        case "builtIn":
            return (nil, icon.name)
        case "resource":
            guard let uri = icon.uri else { return (nil, nil) }
            return (URL(string: uri) ?? URL(fileURLWithPath: uri), nil)
        case "providerAsset":
            guard icon.provider == "lxapp", let appId = icon.key else { return (nil, nil) }
            let path = getLxAppDisplayIconPath(appId).toString()
            return (path.isEmpty ? nil : URL(fileURLWithPath: path), nil)
        default:
            return (nil, nil)
        }
    }

    private static func makeRegistration(
        _ surface: LxAppUIConfig.Surface
    ) throws -> Registration {
        let content: Content
        let presentation: Presentation
        switch surface.content.kind {
        case .lxapp:
            guard let appId = surface.content.appId, !appId.isEmpty else {
                throw LxAppUIError.invalidConfig("main surface \(surface.id) has no appId")
            }
            content = .lxapp(appId: appId, path: try surface.content.resolvedLxAppPath())
            presentation = Presentation(
                automaticTitle: appId,
                icon: .providerAsset(provider: "lxapp", key: appId),
                capabilities: Capabilities(close: false, rename: false)
            )
        case .url:
            guard let url = surface.content.url, !url.isEmpty else {
                throw LxAppUIError.invalidConfig("main surface \(surface.id) has no URL")
            }
            content = .browser(initialUrl: url)
            presentation = Presentation(
                automaticTitle: URL(string: url)?.host ?? url,
                icon: .builtIn("browser"),
                capabilities: Capabilities(close: true, rename: true)
            )
        case .native:
            guard let capability = surface.content.name?.rawValue else {
                throw LxAppUIError.invalidConfig("main surface \(surface.id) has no native capability")
            }
            content = .native(
                capability: capability,
                instanceKey: surface.content.instanceKey
            )
            presentation = Presentation(
                automaticTitle: capability == "terminal" ? "Terminal" : "Browser",
                icon: .builtIn(capability),
                capabilities: Capabilities(close: true, rename: true)
            )
        case .page:
            throw LxAppUIError.unsupported("page main surface \(surface.id) is not supported")
        }
        return Registration(id: surface.id, content: content, presentation: presentation)
    }
}
#endif
