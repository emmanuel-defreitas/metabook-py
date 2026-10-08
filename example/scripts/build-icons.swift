import AppKit

// Package the supplied artwork at standard macOS icon sizes. Keep its aspect
// ratio; transparent margins and rounded corners belong to the icon container.
let sources = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
let resources = URL(fileURLWithPath: CommandLine.arguments[2], isDirectory: true)
let temporary = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
try FileManager.default.createDirectory(at: temporary, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: temporary) }
try FileManager.default.createDirectory(at: resources, withIntermediateDirectories: true)

for (variant, destination) in [("light", "AppIcon"), ("dark", "AppIconDark")] {
    guard let image = NSImage(contentsOf: sources.appendingPathComponent("app-icon-\(variant).png")) else {
        fatalError("Cannot read supplied \(variant) app icon")
    }
    let iconset = temporary.appendingPathComponent("\(destination).iconset")
    try FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)
    for points in [16, 32, 128, 256, 512] {
        for scale in [1, 2] {
            let pixels = points * scale
            let side = CGFloat(pixels)
            let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels,
                bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
            NSGraphicsContext.saveGraphicsState()
            NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
            NSGraphicsContext.current?.imageInterpolation = .high
            NSBezierPath(roundedRect: NSRect(x: 0, y: 0, width: side, height: side),
                xRadius: side * 0.22, yRadius: side * 0.22).addClip()
            let factor = min(side / image.size.width, side / image.size.height)
            let width = image.size.width * factor
            let height = image.size.height * factor
            image.draw(in: NSRect(x: (side - width) / 2, y: (side - height) / 2, width: width, height: height),
                from: .zero, operation: .sourceOver, fraction: 1)
            NSGraphicsContext.restoreGraphicsState()
            let suffix = scale == 2 ? "@2x" : ""
            try bitmap.representation(using: .png, properties: [:])!.write(
                to: iconset.appendingPathComponent("icon_\(points)x\(points)\(suffix).png"))
        }
    }
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
    process.arguments = ["-c", "icns", iconset.path, "-o", resources.appendingPathComponent("\(destination).icns").path]
    try process.run()
    process.waitUntilExit()
    guard process.terminationStatus == 0 else { fatalError("iconutil failed") }
}
