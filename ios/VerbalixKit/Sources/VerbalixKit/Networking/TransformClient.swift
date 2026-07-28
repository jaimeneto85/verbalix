import Foundation

public protocol HTTPTransport: Sendable {
    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse)
}

public struct URLSessionTransport: HTTPTransport {
    private let session: URLSession

    public init(timeout: TimeInterval = 20) {
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = timeout
        self.session = URLSession(configuration: config)
    }

    public func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let (data, response) = try await session.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw VerbalixError.invalidResponse
        }
        return (data, httpResponse)
    }
}

private struct ServerErrorEnvelope: Decodable {
    struct ServerError: Decodable {
        let code: String?
    }
    let error: ServerError?
}

public struct TransformClient: Sendable {
    let transport: HTTPTransport
    let config: BackendConfig

    public init(transport: HTTPTransport = URLSessionTransport(), config: BackendConfig) {
        self.transport = transport
        self.config = config
    }

    public func transform(_ request: TransformRequest, accessToken: String) async throws -> TransformResponse {
        let validated = try request.validated()

        let encoder = JSONEncoder()
        let body = try encoder.encode(validated)

        var urlRequest = URLRequest(url: config.transformEndpoint)
        urlRequest.httpMethod = "POST"
        urlRequest.httpBody = body
        urlRequest.setValue(config.anonKey, forHTTPHeaderField: "apikey")
        urlRequest.setValue("Bearer \(accessToken)", forHTTPHeaderField: "Authorization")
        urlRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")

        let (data, response): (Data, HTTPURLResponse)
        do {
            (data, response) = try await transport.send(urlRequest)
        } catch let error as VerbalixError {
            throw error
        } catch {
            throw VerbalixError.transport(error)
        }

        if response.statusCode == 401 {
            throw VerbalixError.unauthenticated
        }

        if response.statusCode == 504 {
            throw VerbalixError.providerTimeout
        }

        guard (200..<300).contains(response.statusCode) else {
            let mapped = extractServerError(from: data)
            throw mapped
        }

        let decoder = JSONDecoder()
        guard let transformResponse = try? decoder.decode(TransformResponse.self, from: data) else {
            throw VerbalixError.invalidResponse
        }

        return try transformResponse.validated(for: validated)
    }

    private func extractServerError(from data: Data) -> VerbalixError {
        let decoder = JSONDecoder()
        if let envelope = try? decoder.decode(ServerErrorEnvelope.self, from: data),
           let code = envelope.error?.code,
           let mapped = VerbalixError(serverCode: code) {
            return mapped
        }
        return .providerRejected
    }
}
