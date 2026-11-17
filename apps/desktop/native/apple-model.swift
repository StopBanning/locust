// locust-apple-model: Apple's on-device model (Foundation Models), for Locust's "apple" runtime. The same file as the
// Rust app's apps/native/core/helpers/apple-model/main.swift: keep the two alike.
//
//   locust-apple-model --probe   prints {"available":true,"contextSize":N} or {"available":false,"reason":"..."}
//   locust-apple-model           reads ONE JSON request on stdin, {"prompt": "...", "instructions": "..."?},
//                                and writes JSON lines on stdout as the answer comes:
//                                  {"type":"delta","text":"..."}         words added to the answer
//                                  {"type":"replace","text":"..."}       the whole answer so far, when it changed earlier words
//                                  {"type":"done","text":"..."}          the whole answer, last
//                                  {"type":"error","kind":"...","message":"..."}
//
// It keeps nothing: a fresh session per request, no files, no network. It exits when the answer is done, and on its
// own if Locust goes away (its parent becomes launchd), so it never outlives the app.
import Foundation
import FoundationModels

func emit(_ object: [String: Any]) {
  guard let data = try? JSONSerialization.data(withJSONObject: object), var line = String(data: data, encoding: .utf8) else { return }
  line += "\n"
  FileHandle.standardOutput.write(line.data(using: .utf8)!)
}

func reasonWord(_ reason: SystemLanguageModel.Availability.UnavailableReason) -> String {
  switch reason {
  case .deviceNotEligible: return "deviceNotEligible"
  case .appleIntelligenceNotEnabled: return "appleIntelligenceNotEnabled"
  case .modelNotReady: return "modelNotReady"
  @unknown default: return "unknown"
  }
}

func errorKind(_ error: Error) -> String {
  if #available(macOS 27.0, *), let e = error as? LanguageModelError {
    switch e {
    case .contextSizeExceeded: return "contextSizeExceeded"
    case .guardrailViolation: return "guardrailViolation"
    case .refusal: return "refusal"
    case .unsupportedLanguageOrLocale: return "unsupportedLanguageOrLocale"
    case .rateLimited, .timeout: return "rateLimited"
    default: return "other"
    }
  }
  // macOS 26 names them GenerationError.
  if let e = error as? LanguageModelSession.GenerationError {
    switch e {
    case .exceededContextWindowSize: return "contextSizeExceeded"
    case .guardrailViolation: return "guardrailViolation"
    case .refusal: return "refusal"
    case .unsupportedLanguageOrLocale: return "unsupportedLanguageOrLocale"
    case .rateLimited: return "rateLimited"
    case .assetsUnavailable: return "assetsUnavailable"
    case .concurrentRequests: return "rateLimited"
    default: return "other"
    }
  }
  return "other"
}

let model = SystemLanguageModel.default

if CommandLine.arguments.contains("--probe") {
  switch model.availability {
  case .available: emit(["available": true, "contextSize": model.contextSize])
  case .unavailable(let reason): emit(["available": false, "reason": reasonWord(reason)])
  @unknown default: emit(["available": false, "reason": "unknown"])
  }
  exit(0)
}

// Never outlive Locust.
let parent = getppid()
Thread.detachNewThread {
  while true {
    if getppid() != parent { exit(2) }
    Thread.sleep(forTimeInterval: 0.5)
  }
}

let input = FileHandle.standardInput.readDataToEndOfFile()
guard let request = try? JSONSerialization.jsonObject(with: input) as? [String: Any],
      let prompt = request["prompt"] as? String, !prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
  emit(["type": "error", "kind": "badRequest", "message": "The request had no prompt."])
  exit(1)
}
if case .unavailable(let reason) = model.availability {
  emit(["type": "error", "kind": "unavailable", "reason": reasonWord(reason), "message": "Apple's on-device model is not available."])
  exit(1)
}

let instructions = (request["instructions"] as? String).flatMap { $0.isEmpty ? nil : $0 }
let session = instructions.map { LanguageModelSession(model: model, instructions: $0) } ?? LanguageModelSession(model: model)

let done = DispatchSemaphore(value: 0)
var status: Int32 = 0
Task {
  var said = ""
  do {
    // Each element is the whole answer so far, not the words added: send only what changed.
    for try await snapshot in session.streamResponse(to: prompt) {
      let now = snapshot.content
      if now.hasPrefix(said) {
        let added = String(now.dropFirst(said.count))
        if !added.isEmpty { emit(["type": "delta", "text": added]) }
      } else {
        emit(["type": "replace", "text": now])
      }
      said = now
    }
    emit(["type": "done", "text": said])
  } catch {
    emit(["type": "error", "kind": errorKind(error), "message": String(describing: error)])
    status = 1
  }
  done.signal()
}
done.wait()
exit(status)
