// swift-tools-version: 6.0

import PackageDescription
import Foundation

// Get library paths using environment variable or fallback to relative paths
let projectRoot = ProcessInfo.processInfo.environment["LINGXIA_PROJECT_ROOT"] ??
                  URL(fileURLWithPath: #file).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().path

let iosLibraryPath = "\(projectRoot)/target/aarch64-apple-ios/release/liblingxia.a"

// Determine macOS library path based on architecture
#if arch(arm64)
let macosLibraryPath = "\(projectRoot)/target/aarch64-apple-darwin/release/liblingxia.a"
#else
let macosLibraryPath = "\(projectRoot)/target/x86_64-apple-darwin/release/liblingxia.a"
#endif

let package = Package(
    name: "lingxia",
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
                .copy("Resources/icons/icon_back.pdf"),
                .copy("Resources/icons/icon_close.pdf"),
                .copy("Resources/icons/icon_camera_flash_on.pdf"),
                .copy("Resources/icons/icon_camera_flash_off.pdf"),
                .copy("Resources/icons/icon_camera_switch.pdf"),
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
                .linkedFramework("JavaScriptCore"),
                .linkedFramework("WebKit"),
                .linkedFramework("AudioToolbox", .when(platforms: [.iOS])),
                .linkedFramework("CoreLocation", .when(platforms: [.iOS])),
                .linkedFramework("QuickLook", .when(platforms: [.iOS]))
            ]
        ),
    ]
)
