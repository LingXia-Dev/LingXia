// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "{{PROJECT_NAME}}",
    platforms: [
        .macOS(.v12)
    ],
    products: [
        .executable(
            name: "{{SWIFT_TARGET_NAME}}",
            targets: ["{{SWIFT_TARGET_NAME}}"]
        ),
    ],
    dependencies: [
        // Add the LingXia Swift package dependency here before building.
        // Managed by `lingxia build`: the local `.package(path:)` to the cached
        // SDK is injected here automatically (the SDK uses unsafeFlags, so it
        // can only be a local path dependency).
    ],
    targets: [
        .executableTarget(
            name: "{{SWIFT_TARGET_NAME}}",
            dependencies: [
                // .product(name: "lingxia", package: "lingxia"), // managed by `lingxia build`
            ],
            path: "Sources",
            resources: [
                .copy("Resources")
            ]
        ),
    ]
)
