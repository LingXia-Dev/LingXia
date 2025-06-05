// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "miniapp",
    platforms: [
        .iOS(.v17),
    ],
    products: [
        // An xtool project should contain exactly one library product,
        // representing the main app.
        .library(
            name: "miniapp",
            targets: ["miniapp"]
        ),
    ],
    dependencies: [
        .package(path: "../../../ios/lingxia"),
    ],
    targets: [
        .target(
            name: "miniapp",
            dependencies: [
                .product(name: "lingxia", package: "lingxia"),
            ],
            resources: [
                .copy("Resources")
            ]
        ),
    ]
)
