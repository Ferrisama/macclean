// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "MacCleanApp",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(name: "MacCleanApp", targets: ["MacCleanApp"])
    ],
    targets: [
        .executableTarget(
            name: "MacCleanApp",
            path: "Sources/MacCleanApp"
        ),
        .testTarget(
            name: "MacCleanAppTests",
            dependencies: ["MacCleanApp"],
            path: "Tests/MacCleanAppTests"
        )
    ]
)
