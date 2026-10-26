// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "LingXiaGpuDemo",
    platforms: [
        .macOS(.v12)
    ],
    products: [
        .executable(
            name: "LingXiaGpuDemo",
            targets: ["LingXiaGpuDemo"]
        ),
    ],
    dependencies: [
        .package(name: "lingxia", path: "../../../lingxia-sdk/apple"),
    ],
    targets: [
        .executableTarget(
            name: "LingXiaGpuDemo",
            dependencies: [
                "lingxia",
            ],
            path: "Sources",
            resources: [
                .copy("Resources")
            ]
        ),
    ]
)
