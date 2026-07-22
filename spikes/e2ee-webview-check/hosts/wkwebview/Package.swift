// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "FilamentWKWebViewProbe",
    platforms: [.macOS(.v13)],
    products: [
        .executable(name: "FilamentWKWebViewProbe", targets: ["FilamentWKWebViewProbe"]),
    ],
    targets: [
        .executableTarget(name: "FilamentWKWebViewProbe"),
    ],
    swiftLanguageModes: [.v5]
)
