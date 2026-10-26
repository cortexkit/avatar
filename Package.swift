// swift-tools-version:5.9
//
// This manifest lives at the repository root, not under ports/swift/, because
// SwiftPM can only consume a git dependency whose manifest sits at the clone
// root: the iOS consumer takes AvatarKit by git URL, and a manifest under
// ports/ would force every consumer onto path dependencies or vendoring. The
// sources themselves stay in ports/swift/ beside the other language ports;
// the targets below point at them explicitly.
import PackageDescription

let package = Package(
    name: "AvatarKit",
    platforms: [.iOS(.v13), .macOS(.v11)],
    products: [
        .library(name: "AvatarKit", targets: ["AvatarKit"])
    ],
    targets: [
        .binaryTarget(
            name: "AvatarCore",
            path: "ports/swift/AvatarCore.xcframework"
        ),
        .target(
            name: "AvatarKit",
            dependencies: [
                .target(name: "AvatarCore", condition: .when(platforms: [.iOS, .macOS])),
            ],
            path: "ports/swift/Sources/AvatarKit"
        ),
        .testTarget(
            name: "AvatarKitTests",
            dependencies: ["AvatarKit"],
            path: "ports/swift/Tests/AvatarKitTests"
        ),
    ]
)
