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
    ],
    targets: [
        .executableTarget(
            name: "{{SWIFT_TARGET_NAME}}",
            dependencies: [
                // "lingxia",
            ],
            path: "Sources",
            resources: [
                .copy("Resources")
            ]
        ),
    ]
)
