// swift-tools-version: 6.0

import PackageDescription
import Foundation

// Get the current directory and construct the path to the library
let currentDirectory = FileManager.default.currentDirectoryPath
let libraryPath = "\(currentDirectory)/../../../../target/aarch64-apple-ios/release/liblingxia.a"

let package = Package(
    name: "lingxia",
    platforms: [
        .iOS(.v17),
    ],
    products: [
        // An xtool project should contain exactly one library product,
        // representing the main app.
        .library(
            name: "lingxia",
            targets: ["lingxia"]
        ),
    ],
    targets: [
        .target(
            name: "lingxia",
            linkerSettings: [
                .unsafeFlags([libraryPath], .when(platforms: [.iOS])),
            ]
        ),
    ]
)
