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
    dependencies: [
        .package(url: "https://github.com/supabase/supabase-swift.git", from: "2.0.0")
    ],
    targets: [
        .target(
            name: "VerbalixKit",
            dependencies: [
                .product(name: "Auth", package: "supabase-swift")
            ],
            path: "Sources/VerbalixKit"
        ),
        .testTarget(
            name: "VerbalixKitTests",
            dependencies: ["VerbalixKit"],
            path: "Tests/VerbalixKitTests"
        )
    ]
)
