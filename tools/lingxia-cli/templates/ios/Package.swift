// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "{{PROJECT_NAME}}",
    platforms: [
        .iOS(.v17)
    ],
    products: [
        .library(
            name: "{{SWIFT_TARGET_NAME}}",
            targets: ["{{SWIFT_TARGET_NAME}}"]
        ),
    ],
    dependencies: [
        // Add the LingXia Swift package dependency here before building.
        // `lingxia build` injects it: a local `.package(path:)` to the cached SDK
        // (unsafeFlags rules out a remote URL). The path is machine-local and is
        // rewritten on every build, so this line always shows as a local diff.
    ],
    targets: [
        .target(
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
