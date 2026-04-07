// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "LingXiaRunner",
    platforms: [
        .macOS(.v12)
    ],
    products: [
        .executable(
            name: "LingXiaRunner",
            targets: ["LingXiaRunner"]
        ),
    ],
    dependencies: [
        .package(name: "lingxia", path: "../../lingxia-sdk/apple"),
    ],
    targets: [
        .executableTarget(
            name: "LingXiaRunner",
            dependencies: [
                .product(name: "lingxia", package: "lingxia"),
            ],
            path: "Sources",
            resources: [
                .copy("Resources")
            ],
            plugins: [
                .plugin(name: "RunnerBuildPlugin")
            ]
        ),
        .plugin(
            name: "RunnerBuildPlugin",
            capability: .buildTool(),
            dependencies: ["RunnerBuildTool"],
            path: "plugins/RunnerBuildPlugin"
        ),
        .executableTarget(
            name: "RunnerBuildTool",
            path: "plugins/RunnerBuildTool"
        ),
    ]
)
