// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "xcode_add_share_extension",
    platforms: [.macOS(.v13)],
    dependencies: [
        .package(url: "https://github.com/tuist/XcodeProj.git", from: "8.16.0"),
    ],
    targets: [
        .executableTarget(
            name: "xcode_add_share_extension",
            dependencies: ["XcodeProj"],
            path: "Sources"
        ),
    ]
)
