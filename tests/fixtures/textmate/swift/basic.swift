import Foundation

// Swift basic fixture: café, 東京, λ, 🚀, and astral 𝌆.
enum Stage: String {
    case queued, ready, complete
}

struct Launch<Payload: CustomStringConvertible> {
    let name: String
    let stage: Stage
    let payload: Payload

    func summary(prefix: String) -> String {
        return "\(prefix) \(name): \(payload) 🚀 𝌆"
    }
}

func describe(_ stage: Stage) -> String {
    switch stage {
    case .queued: return "waiting"
    case .ready: return "ready"
    case .complete: return "done"
    }
}

print(Launch(name: "café 東京 λ", stage: .ready, payload: 42).summary(prefix: "Mission"))
