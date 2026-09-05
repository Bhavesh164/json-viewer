// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "JSONViewer",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(name: "JSONViewer", targets: ["JSONViewer"]),
        .executable(name: "JSONViewerTests", targets: ["JSONViewerTests"])
    ],
    targets: [
        .target(
            name: "JSONViewerCore",
            path: "Sources/JSONViewerCore"
        ),
        .executableTarget(
            name: "JSONViewer",
            dependencies: ["JSONViewerCore"],
            path: "Sources/JSONViewer"
        ),
        .executableTarget(
            name: "JSONViewerTests",
            dependencies: ["JSONViewerCore"],
            path: "Tests/JSONViewerTests"
        )
    ]
)
