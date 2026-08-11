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
    products: [
        .library(name: "AvatarKit", targets: ["AvatarKit"])
    ],
    targets: [
        // Vendored BLAKE3 reference implementation, portable-only build.
        // The BLAKE3_NO_* defines disable every x86 SIMD path; the upstream
        // NEON path keys off BLAKE3_USE_NEON (autodetected as 1 on AArch64),
        // so it must be pinned to 0 explicitly. See Sources/CBlake3/VENDORED.md.
        .target(
            name: "CBlake3",
            path: "ports/swift/Sources/CBlake3",
            publicHeadersPath: "include",
            cSettings: [
                .define("BLAKE3_NO_SSE2"),
                .define("BLAKE3_NO_SSE41"),
                .define("BLAKE3_NO_AVX2"),
                .define("BLAKE3_NO_AVX512"),
                .define("BLAKE3_NO_NEON"),
                .define("BLAKE3_USE_NEON", to: "0"),
            ]
        ),
        .target(
            name: "AvatarKit",
            dependencies: ["CBlake3"],
            path: "ports/swift/Sources/AvatarKit"
        ),
        .testTarget(
            name: "AvatarKitTests",
            dependencies: ["AvatarKit"],
            path: "ports/swift/Tests/AvatarKitTests"
        ),
    ]
)
