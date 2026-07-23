import AVFoundation
import AppKit

let arguments = CommandLine.arguments
guard arguments.count >= 3 else {
    print("Usage: video_thumbnail <source-path> <output-path>")
    exit(1)
}

let sourcePath = arguments[1]
let outputPath = arguments[2]
let sourceURL = URL(fileURLWithPath: sourcePath)
let destURL = URL(fileURLWithPath: outputPath)

let ext = sourceURL.pathExtension.lowercased()
let isVideo = ["mp4", "webm", "ogg", "mov", "m4v"].contains(ext)

do {
    if isVideo {
        let asset = AVAsset(url: sourceURL)
        let generator = AVAssetImageGenerator(asset: asset)
        generator.appliesPreferredTrackTransform = true
        generator.maximumSize = CGSize(width: 120, height: 120)
        let time = CMTime(seconds: 0.5, preferredTimescale: 600)
        let imageRef = try generator.copyCGImage(at: time, actualTime: nil)
        let rep = NSBitmapImageRep(cgImage: imageRef)
        guard let data = rep.representation(using: .jpeg, properties: [.compressionFactor: 0.7]) else {
            exit(1)
        }
        try data.write(to: destURL)
    } else {
        // Image
        guard let image = NSImage(contentsOf: sourceURL) else {
            exit(1)
        }
        let maxDimension: CGFloat = 120.0
        let size = image.size
        if size.width == 0 || size.height == 0 {
            exit(1)
        }
        let ratio = size.width / size.height
        var newSize = NSSize(width: maxDimension, height: maxDimension)
        if ratio > 1.0 {
            newSize.height = maxDimension / ratio
        } else {
            newSize.width = maxDimension * ratio
        }

        let newImage = NSImage(size: newSize)
        newImage.lockFocus()
        image.draw(in: NSRect(origin: .zero, size: newSize), from: NSRect(origin: .zero, size: size), operation: .copy, fraction: 1.0)
        newImage.unlockFocus()

        guard let tiffData = newImage.tiffRepresentation,
              let bitmap = NSBitmapImageRep(data: tiffData),
              let jpegData = bitmap.representation(using: .jpeg, properties: [.compressionFactor: 0.7]) else {
            exit(1)
        }
        try jpegData.write(to: destURL)
    }
    print("Success")
} catch {
    print("Error: \(error)")
    exit(1)
}
