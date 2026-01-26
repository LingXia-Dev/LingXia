// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "lxapp",
    platforms: [
        .iOS(.v17),
        .macOS(.v12)
    ],
    products: [
        // An xtool project should contain exactly one library product,
        // representing the main app.
        .library(
            name: "lxapp",
            targets: ["lxapp"]
        ),
    ],
    dependencies: [
        // In dev, `examples/ios/dev.sh` stages the Apple SDK under `target/spm/lingxia`
        .package(name: "lingxia", path: "../../../target/spm/lingxia"),
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
