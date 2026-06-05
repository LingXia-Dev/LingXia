// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "RustLogicDemo",
    platforms: [
        .macOS(.v12)
    ],
    products: [
        .executable(
            name: "RustLogicDemo",
            targets: ["RustLogicDemo"]
        ),
    ],
    dependencies: [
        .package(name: "lingxia", path: "../../../lingxia-sdk/apple"),
    ],
    targets: [
        .executableTarget(
            name: "RustLogicDemo",
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
