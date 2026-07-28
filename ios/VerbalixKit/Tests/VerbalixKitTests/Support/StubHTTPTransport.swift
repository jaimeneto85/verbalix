import Foundation
@testable import VerbalixKit

struct StubHTTPTransportError: Error {}

final class StubHTTPTransport: HTTPTransport, @unchecked Sendable {
    private let lock = NSLock()
    private var responses: [Result<(Data, Int), Error>]
    private(set) var capturedRequests: [URLRequest] = []

    init(responses: [Result<(Data, Int), Error>]) {
        self.responses = responses
    }

    static func success(status: Int, body: Data) -> StubHTTPTransport {
        StubHTTPTransport(responses: [.success((body, status))])
    }

    static func failure(_ error: Error) -> StubHTTPTransport {
        StubHTTPTransport(responses: [.failure(error)])
    }

    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let next: Result<(Data, Int), Error> = try lock.withLock {
            capturedRequests.append(request)
            guard !responses.isEmpty else {
                throw StubHTTPTransportError()
            }
            return responses.removeFirst()
        }

        switch next {
        case let .success((data, status)):
            let response = HTTPURLResponse(
                url: request.url ?? URL(string: "https://example.com")!,
                statusCode: status,
                httpVersion: nil,
                headerFields: nil
            )!
            return (data, response)
        case let .failure(error):
            throw error
        }
    }

    var lastRequestBody: Data? {
        capturedRequests.last?.httpBody
    }
}
