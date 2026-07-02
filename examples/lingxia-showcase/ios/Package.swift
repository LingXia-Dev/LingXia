// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "lxapp",
    platforms: [
        .iOS(.v17),
        .macOS(.v12)
    ],
    products: [
        // Exactly one library product,
        // representing the main app.
        .library(
            name: "lxapp",
            targets: ["lxapp"]
        ),
    ],
    dependencies: [
        .package(name: "lingxia", path: "../../../lingxia-sdk/apple"),
    ],
    targets: [
        .target(
            name: "lxapp",
            dependencies: [
                .product(name: "lingxia", package: "lingxia"),
            ],
            resources: [
                .copy("Resources")
            ]
        ),
    ]
)
