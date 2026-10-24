// Local, offline OCR. Results are observations, not policy authority.
import Foundation
import Vision
import ImageIO

var output: [[String: Any]] = []
for file in CommandLine.arguments.dropFirst() {
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.usesLanguageCorrection = false
    request.recognitionLanguages = ["en-US"]
    let handler = VNImageRequestHandler(url: URL(fileURLWithPath: file), options: [:])
    try handler.perform([request])
    let lines = (request.results ?? []).compactMap { observation -> [String: Any]? in
        guard let candidate = observation.topCandidates(1).first else { return nil }
        return ["text": candidate.string, "confidence": candidate.confidence,
                "x": observation.boundingBox.minX, "y": observation.boundingBox.minY]
    }
    output.append(["frame": URL(fileURLWithPath: file).lastPathComponent, "lines": lines])
}
let data = try JSONSerialization.data(withJSONObject: output, options: [.sortedKeys])
FileHandle.standardOutput.write(data)
