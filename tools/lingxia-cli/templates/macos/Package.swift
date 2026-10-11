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
        // `lingxia build` prepares this ignored link to the selected cached SDK.
        .package(name: "lingxia", path: "../.lingxia/sdk/apple"), // lingxia-sdk: managed by `lingxia build`
    ],
    targets: [
        .executableTarget(
            name: "{{SWIFT_TARGET_NAME}}",
            dependencies: [
                .product(name: "lingxia", package: "lingxia"),
            ],
            path: "Sources",
            resources: [
                .copy("Resources")
            ]
        ),
    ]
)
