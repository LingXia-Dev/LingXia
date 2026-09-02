// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "{{PROJECT_NAME}}",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .executable(
            name: "{{SWIFT_TARGET_NAME}}",
            targets: ["{{SWIFT_TARGET_NAME}}"]
        ),
    ],
    dependencies: [
        // Add the LingXia Swift package dependency here before building.
        // `lingxia build` temporarily injects a local `.package(path:)` to the
        // cached SDK (unsafeFlags rules out a remote URL), then restores this
        // manifest so machine-local paths never remain in the source tree.
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
