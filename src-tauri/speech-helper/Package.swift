// swift-tools-version: 6.0
import PackageDescription
import Foundation

// The Info.plist is embedded in the binary, so macOS can show the microphone and speech prompts for it.
let infoPlist = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("Info.plist").path

let package = Package(
    name: "speech-helper",
    platforms: [.macOS("26.0")],
    dependencies: [
        // Parakeet speech recognition on the Apple Neural Engine. Pinned, so a build always gets the same API.
        .package(url: "https://github.com/FluidInference/FluidAudio.git", exact: "0.17.7"),
    ],
    targets: [
        .executableTarget(
            name: "speech-helper",
            dependencies: [.product(name: "FluidAudio", package: "FluidAudio")],
            path: "Sources/speech-helper",
            // The helper predates Swift 6 concurrency checking. Its shared state is guarded by locks, so it builds in Swift 5 mode.
            swiftSettings: [.swiftLanguageMode(.v5)],
            linkerSettings: [
                .unsafeFlags(["-Xlinker", "-sectcreate", "-Xlinker", "__TEXT", "-Xlinker", "__info_plist", "-Xlinker", infoPlist]),
            ]
        ),
    ]
)
