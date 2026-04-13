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
    ],
    targets: [
        .target(
            name: "{{SWIFT_TARGET_NAME}}",
            dependencies: [
                // .product(name: "lingxia", package: "lingxia"),
            ],
            path: "Sources",
            resources: [
                .copy("Resources")
            ]
        ),
    ]
)
