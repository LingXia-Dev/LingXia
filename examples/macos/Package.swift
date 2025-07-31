// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "LingXiaDemo",
    platforms: [
        .macOS(.v11)
    ],
    products: [
        .executable(
            name: "LingXiaDemo",
            targets: ["LingXiaDemo"]
        ),
    ],
    dependencies: [
        .package(name: "lingxia", path: "../../../lingxia-sdk/apple"),
    ],
    targets: [
        .executableTarget(
            name: "LingXiaDemo",
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
