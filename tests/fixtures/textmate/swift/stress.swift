#!/usr/bin/swift
// Swift parity stress fixture: café, λ, 🚀, and the astral symbol 𝌆.
import Foundation
import struct Foundation.Date
@testable import MarkSupport
#if canImport(RegexBuilder)
import RegexBuilder
#endif
// MARK: - Operators, protocols, and generic relationships

precedencegroup ForwardApplicationPrecedence {
    associativity: left
    higherThan: AssignmentPrecedence
}
infix operator |>: ForwardApplicationPrecedence
func |> (value: String, transform: (String) -> String) -> String {
    transform(value)
}
@available(macOS 13, iOS 16, *)
public protocol Identified: Sendable {
    associatedtype Identifier: Hashable & Sendable
    var id: Identifier { get }
    static var kind: String { get }
    func summary(prefix: String) async throws -> String
}
extension Identified {
    static var kind: String { String(describing: Self.self) }
    func summary(prefix: String) async throws -> String {
        "\(prefix) \(Self.kind): \(id)"
    }
}
func eraseDescription(
    _ value: some CustomStringConvertible
) -> any CustomStringConvertible {
    value
}
// MARK: - Enums, structures, classes, and actors

indirect enum SyntaxNode {
    case token(String, location: (line: Int, column: Int))
    case group(name: String?, children: [SyntaxNode])
    case reference(AnyKeyPath)
    case missing
}
struct Document: Sendable {
    let id: UUID
    var title: String {
        willSet { print("new title: \(newValue)") }
        didSet { print("old title: \(oldValue)") }
    }
    var subtitle: String?
    private(set) var tags: [String]
    var metadata: [String: String]
    var displayTitle: String { title.uppercased() }
    subscript(tag index: Int) -> String {
        tags[index]
    }
    mutating func removeTags() {
        tags.removeAll()
    }
}
@dynamicMemberLookup
final class RenderSession: @unchecked Sendable {
    var prefix: String { "→" }
    required init() {}
    subscript(dynamicMember member: String) -> String {
        "\(prefix)\(member)"
    }
    deinit { print("render session closed") }
}
actor DocumentStore {
    nonisolated var implementation: String { "actor-backed" }
    func save(_ document: consuming Document) async throws {
        print(document.displayTitle)
    }
    func fetch(_ id: UUID) async -> Document? {
        nil
    }
}
// MARK: - Property wrappers and result builders

@propertyWrapper
struct Uppercased {
    var wrappedValue: String { "WRAPPED" }
    init(wrappedValue: String) {
        print(wrappedValue.uppercased())
    }
}
struct Settings {
    @Uppercased var mode: String
    var note: String?
}
@resultBuilder
enum LineBuilder {
    static func buildBlock(_ parts: String...) -> [String] { parts }
    static func buildExpression(_ expression: String) -> String { expression }
}
func lines(@LineBuilder _ content: () -> [String]) -> [String] {
    content()
}
func launchLines() -> [String] {
    lines {
        "Mission 🚀"
        "Telemetry 𝌆"
        "Ready"
    }
}
// MARK: - Optionals, patterns, loops, and errors

func unwrap(title: String?) -> String {
    if let title {
        return title
    }
    return "untitled"
}
func classify(_ value: Any?) -> String {
    switch value {
    case nil:
        return "none"
    case let integer as Int:
        return "integer \(integer)"
    case let text as String where text.isEmpty:
        return "empty"
    case is Double:
        return "floating point"
    default:
        return "other"
    }
}
func describe(_ node: SyntaxNode) -> String {
    switch node {
    case let .token(text, location):
        return "\(text) at \(location.line):\(location.column)"
    case let .group(name, children) where children.isEmpty:
        return unwrap(title: name)
    case let .group(name, children):
        return "\(unwrap(title: name)) has \(children.count) children"
    case .reference:
        return "key path"
    case .missing:
        return "missing"
    }
}
enum ParseError: Error {
    case malformed(String)
}
func parsePair(_ input: String) throws -> (name: String, count: Int) {
    if input.isEmpty {
        throw ParseError.malformed(input)
    }
    return (input, input.count)
}
func visit(_ values: [String]) {
    defer { print("visit complete") }
    for value in values where value.isEmpty {
        continue
    }
}
// MARK: - Closures, key paths, and concurrency

@discardableResult
func apply(_ text: String, body: @escaping (String) -> String) -> String {
    body(text)
}
func closureDemo(_ documents: [Document]) {
    print(apply("swift") { value in value.uppercased() })
    print(documents.map { document in document.title })
    print(documents.map(\.title))
    print(\Document.title)
    print("swift" |> { $0.capitalized })
}
@MainActor
func asynchronousDemo(store: DocumentStore) async {
    await store.fetch(UUID())
    try? await store.save(Document(id: UUID(), title: "Grammar",
                                   subtitle: nil, tags: ["Swift", "Unicode"],
                                   metadata: ["symbol": "🚀"]))
}
// MARK: - Strings, comments, regex, and directives

/* A multiline comment keeps state across lines.
   Nested comments are legal in Swift: /* inner café */
   Both levels close before declarations resume. */
func escapedText() -> String {
    "quote: \"; slash: \\; scalar: \u{1F680}; value: \(classify(3))"
}
func rawText() -> String {
    #"raw \#(escapedText()) keeps \n and contains "quotes" plus 𝌆"#
}
func multilineText() -> String {
    """
    First line with café.
      Second line interpolates: \(launchLines().joined(separator: ", ")).
    A closing-looking sequence is escaped: \"\"\"
    """
}
func rawMultilineText() -> String {
    ##"""
    Raw path C:\temp\new and interpolation \##(classify("Swift")).
    One hash is inert: \#(escapedText()); symbols stay literal: 🚀 𝌆.
    """##
}
@available(macOS 13, iOS 16, *)
func containsSwift(_ text: String) -> Bool {
    text.contains(/(?i)swift|textmate|café/)
}
#if DEBUG
func buildConfiguration() -> String { "debug" }
#elseif os(Linux)
func buildConfiguration() -> String { "linux-release" }
#else
func buildConfiguration() -> String { "release" }
#endif
do {
    try parsePair("tokens:3")
} catch let error as LocalizedError {
    print(error.errorDescription)
} catch {
    print("parse failed: \(error)")
}
