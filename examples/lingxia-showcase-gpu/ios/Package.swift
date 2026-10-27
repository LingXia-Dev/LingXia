// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "gpu",
    platforms: [
        .iOS(.v17),
        .macOS(.v12)
    ],
    products: [
        // Exactly one library product,
        // representing the main app.
        .library(
            name: "gpu",
            targets: ["gpu"]
        ),
    ],
    dependencies: [
        .package(name: "lingxia", path: "../../../lingxia-sdk/apple"),
    ],
    targets: [
        .target(
            name: "gpu",
            dependencies: [
                .product(name: "lingxia", package: "lingxia"),
            ],
            resources: [
                .copy("Resources")
            ],
            // GPU pages present through wgpu's Metal backend.
            linkerSettings: [
                .linkedFramework("Metal"),
                .linkedFramework("QuartzCore"),
            ]
        ),
    ]
)
