import Foundation

public struct HistoryItem: Codable, Sendable {
    public let id: UUID
    public let operation: String
    public let sourceText: String
    public let resultText: String
    public let sourceLanguage: String
    public let targetLanguage: String?
    public let createdAt: String

    enum CodingKeys: String, CodingKey {
        case id
        case operation
        case sourceText = "source_text"
        case resultText = "result_text"
        case sourceLanguage = "source_language"
        case targetLanguage = "target_language"
        case createdAt = "created_at"
    }

    public init(id: UUID, operation: String, sourceText: String, resultText: String, sourceLanguage: String, targetLanguage: String?, createdAt: String) {
        self.id = id
        self.operation = operation
        self.sourceText = sourceText
        self.resultText = resultText
        self.sourceLanguage = sourceLanguage
        self.targetLanguage = targetLanguage
        self.createdAt = createdAt
    }
}

public struct HistoryInsert: Codable, Sendable {
    public let operation: String
    public let sourceText: String
    public let resultText: String
    public let sourceLanguage: String
    public let targetLanguage: String?

    enum CodingKeys: String, CodingKey {
        case operation
        case sourceText = "source_text"
        case resultText = "result_text"
        case sourceLanguage = "source_language"
        case targetLanguage = "target_language"
    }

    public init(operation: String, sourceText: String, resultText: String, sourceLanguage: String, targetLanguage: String?) {
        self.operation = operation
        self.sourceText = sourceText
        self.resultText = resultText
        self.sourceLanguage = sourceLanguage
        self.targetLanguage = targetLanguage
    }
}

public struct HistoryClient: Sendable {
    let transport: HTTPTransport
    let config: BackendConfig

    public init(transport: HTTPTransport = URLSessionTransport(), config: BackendConfig) {
        self.transport = transport
        self.config = config
    }

    public func insert(_ item: HistoryInsert, accessToken: String) async throws {
        let encoder = JSONEncoder()
        let body = try encoder.encode(item)

        var request = URLRequest(url: config.historyEndpoint)
        request.httpMethod = "POST"
        request.httpBody = body
        request.setValue(config.anonKey, forHTTPHeaderField: "apikey")
        request.setValue("Bearer \(accessToken)", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("return=minimal", forHTTPHeaderField: "Prefer")

        let (_, response) = try await sendRequest(request)
        try validateStatus(response)
    }

    public func list(accessToken: String) async throws -> [HistoryItem] {
        var components = URLComponents(url: config.historyEndpoint, resolvingAgainstBaseURL: false)!
        components.queryItems = [
            URLQueryItem(name: "select", value: "*"),
            URLQueryItem(name: "order", value: "created_at.desc")
        ]

        var request = URLRequest(url: components.url!)
        request.httpMethod = "GET"
        request.setValue(config.anonKey, forHTTPHeaderField: "apikey")
        request.setValue("Bearer \(accessToken)", forHTTPHeaderField: "Authorization")

        let (data, response) = try await sendRequest(request)
        try validateStatus(response)

        let decoder = JSONDecoder()
        guard let items = try? decoder.decode([HistoryItem].self, from: data) else {
            throw VerbalixError.invalidResponse
        }
        return items
    }

    public func delete(id: UUID, accessToken: String) async throws {
        var components = URLComponents(url: config.historyEndpoint, resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "id", value: "eq.\(id.uuidString)")]

        var request = URLRequest(url: components.url!)
        request.httpMethod = "DELETE"
        request.setValue(config.anonKey, forHTTPHeaderField: "apikey")
        request.setValue("Bearer \(accessToken)", forHTTPHeaderField: "Authorization")
        request.setValue("return=minimal", forHTTPHeaderField: "Prefer")

        let (_, response) = try await sendRequest(request)
        try validateStatus(response)
    }

    public func deleteAll(accessToken: String) async throws {
        var components = URLComponents(url: config.historyEndpoint, resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "id", value: "not.is.null")]

        var request = URLRequest(url: components.url!)
        request.httpMethod = "DELETE"
        request.setValue(config.anonKey, forHTTPHeaderField: "apikey")
        request.setValue("Bearer \(accessToken)", forHTTPHeaderField: "Authorization")
        request.setValue("return=minimal", forHTTPHeaderField: "Prefer")

        let (_, response) = try await sendRequest(request)
        try validateStatus(response)
    }

    private func sendRequest(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        do {
            return try await transport.send(request)
        } catch let error as VerbalixError {
            throw error
        } catch {
            throw VerbalixError.transport(error)
        }
    }

    private func validateStatus(_ response: HTTPURLResponse) throws {
        if response.statusCode == 401 {
            throw VerbalixError.unauthenticated
        }
        guard (200..<300).contains(response.statusCode) else {
            throw VerbalixError.providerRejected
        }
    }
}
