// swift-tools-version: 6.0

import PackageDescription
import Foundation

func findProjectRoot() -> String {
    let fm = FileManager.default

    func isWorkspaceRoot(_ path: String) -> Bool {
        fm.fileExists(atPath: "\(path)/Cargo.toml")
            && fm.fileExists(atPath: "\(path)/crates/lingxia/Cargo.toml")
    }

    func firstWorkspaceRoot(startingAt path: String) -> String? {
        var url = URL(fileURLWithPath: path).standardizedFileURL
        var isDirectory: ObjCBool = false
        if !fm.fileExists(atPath: url.path, isDirectory: &isDirectory) {
            return nil
        }
        if !isDirectory.boolValue {
            url.deleteLastPathComponent()
        }

        while true {
            if isWorkspaceRoot(url.path) {
                return url.path
            }
            let parent = url.deletingLastPathComponent()
            if parent.path == url.path {
                return nil
            }
            url = parent
        }
    }

    let candidates: [String] = [
        ProcessInfo.processInfo.environment["LINGXIA_PROJECT_ROOT"],
        URL(fileURLWithPath: #filePath).deletingLastPathComponent().path,
        fm.currentDirectoryPath,
    ].compactMap { $0 }

    for candidate in candidates {
        if let root = firstWorkspaceRoot(startingAt: candidate) {
            return root
        }
    }

    return URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .path
}

let projectRoot = findProjectRoot()

let buildConfig = ProcessInfo.processInfo.environment["LINGXIA_BUILD_CONFIG"] ?? "release"
let runnerTargetTriple = ProcessInfo.processInfo.environment["RUNNER_TARGET_TRIPLE"]?.lowercased()

let iosLibraryPath = "\(projectRoot)/target/aarch64-apple-ios/\(buildConfig)/liblingxia.a"

func resolveMacosLibraryPath() -> String {
    if let triple = runnerTargetTriple {
        if triple.hasPrefix("arm64-apple-macosx") || triple.hasPrefix("aarch64-apple-macosx")
            || triple == "aarch64-apple-darwin" || triple == "arm64-apple-darwin"
        {
            return "\(projectRoot)/target/aarch64-apple-darwin/\(buildConfig)/liblingxia.a"
        }
        if triple.hasPrefix("x86_64-apple-macosx") || triple == "x86_64-apple-darwin" {
            return "\(projectRoot)/target/x86_64-apple-darwin/\(buildConfig)/liblingxia.a"
        }
    }

    #if arch(arm64)
    return "\(projectRoot)/target/aarch64-apple-darwin/\(buildConfig)/liblingxia.a"
    #else
    return "\(projectRoot)/target/x86_64-apple-darwin/\(buildConfig)/liblingxia.a"
    #endif
}

let macosLibraryPath = resolveMacosLibraryPath()

let package = Package(
    name: "lingxia",
     defaultLocalization: "en",
    platforms: [
        .iOS(.v17),
        .macOS(.v12)
    ],
    products: [
        .library(
            name: "lingxia",
            targets: ["lingxia"]
        ),
    ],
    targets: [
        .systemLibrary(
            name: "CLingXiaRustAPI",
            path: "Sources/generated/LingXiaRustAPI"
        ),
        .systemLibrary(
            name: "CLingXiaSwiftAPI",
            path: "Sources/generated/LingXiaSwiftAPI"
        ),
        .target(
            name: "lingxia",
            dependencies: ["CLingXiaRustAPI", "CLingXiaSwiftAPI"],
            path: "Sources",
            resources: [
                .copy("Resources/icons"),
                .copy("Resources/favicon.ico"),
                .process("Resources/en.lproj"),
                .process("Resources/zh-Hans.lproj"),
            ],
            publicHeadersPath: nil,
            cSettings: [
                .headerSearchPath("generated"),
                .headerSearchPath("generated/LingXiaRustAPI"),
                .headerSearchPath("generated/LingXiaSwiftAPI"),
            ],
            linkerSettings: [
                .unsafeFlags([iosLibraryPath], .when(platforms: [.iOS])),
                .unsafeFlags([macosLibraryPath], .when(platforms: [.macOS])),
                .unsafeFlags(["-Xlinker", "-u", "-Xlinker", "_lingxia_install_host_addon"], .when(platforms: [.iOS])),
                .unsafeFlags(["-Xlinker", "-u", "-Xlinker", "_lingxia_install_host_addon"], .when(platforms: [.macOS])),
                .linkedFramework("JavaScriptCore"),
                .linkedFramework("WebKit"),
                .linkedFramework("AudioToolbox", .when(platforms: [.iOS])),
                .linkedFramework("CoreLocation", .when(platforms: [.iOS])),
                .linkedFramework("QuickLook", .when(platforms: [.iOS])),
                .linkedFramework("PhotosUI", .when(platforms: [.iOS]))
            ]
        ),
    ]
)
