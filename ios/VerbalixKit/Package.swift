// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "VerbalixKit",
    platforms: [
        .iOS(.v17),
        .macOS(.v14)
    ],
    products: [
        .library(name: "VerbalixKit", targets: ["VerbalixKit"])
    ],
    targets: [
        .target(
            name: "VerbalixKit",
            dependencies: [],
            path: "Sources/VerbalixKit"
        ),
        .testTarget(
            name: "VerbalixKitTests",
            dependencies: ["VerbalixKit"],
            path: "Tests/VerbalixKitTests"
        )
    ]
)
