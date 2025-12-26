import Foundation
import Vision
import AppKit

@_cdecl("extract_text_from_image")
public func extractTextFromImage(
    imageData: UnsafePointer<UInt8>,
    imageLength: Int
) -> UnsafeMutablePointer<CChar>? {
    let data = Data(bytes: imageData, count: imageLength)
    
    guard let image = NSImage(data: data),
          let cgImage = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
        return strdup("")
    }
    
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.usesLanguageCorrection = true
    
    let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
    
    do {
        try handler.perform([request])
    } catch {
        return strdup("")
    }
    
    guard let observations = request.results else {
        return strdup("")
    }
    
    // Sort by position (top to bottom, left to right)
    let sortedObservations = observations.sorted { a, b in
        let aY = 1 - a.boundingBox.origin.y  // Vision uses bottom-left origin
        let bY = 1 - b.boundingBox.origin.y
        if abs(aY - bY) > 0.02 {  // Different lines
            return aY < bY
        }
        return a.boundingBox.origin.x < b.boundingBox.origin.x
    }
    
    let text = sortedObservations
        .compactMap { $0.topCandidates(1).first?.string }
        .joined(separator: "\n")
    
    return strdup(text)
}

@_cdecl("free_ocr_string")
public func freeOcrString(ptr: UnsafeMutablePointer<CChar>?) {
    if let ptr = ptr {
        free(ptr)
    }
}
