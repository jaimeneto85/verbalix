import XCTest
@testable import VerbalixKit

final class TransformClientTests: XCTestCase {
    private let config = BackendConfig(supabaseURL: URL(string: "https://project.supabase.co")!, anonKey: "anon-key")!
    private let requestId = UUID(uuidString: "b65c8888-fb0e-4a8f-9fee-95268995bf68")!

    private func makeRequest() -> TransformRequest {
        TransformRequest(requestId: requestId, operation: .translate, text: "Translate this")
    }

    private func responseBody(targetLanguage: String? = "Portuguese", result: String = "Resultado") -> Data {
        let response = TransformResponse(
            requestId: requestId,
            sourceLanguage: "English",
            targetLanguage: targetLanguage,
            result: result
        )
        return try! JSONEncoder().encode(response)
    }

    private func errorBody(code: String) -> Data {
        "{\"error\":{\"code\":\"\(code)\"}}".data(using: .utf8)!
    }

    func test200RespondsWithAValidatedTransformResponse() async throws {
        let transport = StubHTTPTransport.success(status: 200, body: responseBody())
        let client = TransformClient(transport: transport, config: config)

        let response = try await client.transform(makeRequest(), accessToken: "token")

        XCTAssertEqual(response.requestId, requestId)
        XCTAssertEqual(response.result, "Resultado")

        let sentRequest = try XCTUnwrap(transport.capturedRequests.first)
        XCTAssertEqual(sentRequest.value(forHTTPHeaderField: "apikey"), "anon-key")
        XCTAssertEqual(sentRequest.value(forHTTPHeaderField: "Authorization"), "Bearer token")
    }

    func test401MapsToUnauthenticatedRegardlessOfBody() async {
        let transport = StubHTTPTransport.success(status: 401, body: errorBody(code: "TEXT_TOO_LONG"))
        let client = TransformClient(transport: transport, config: config)

        await assertThrows(client: client, expected: .unauthenticated)
    }

    func test413MapsToTextTooLongFromTheServerErrorBody() async {
        let transport = StubHTTPTransport.success(status: 413, body: errorBody(code: "TEXT_TOO_LONG"))
        let client = TransformClient(transport: transport, config: config)

        await assertThrows(client: client, expected: .textTooLong)
    }

    func test429MapsToRateLimitedFromTheServerErrorBody() async {
        let transport = StubHTTPTransport.success(status: 429, body: errorBody(code: "RATE_LIMITED"))
        let client = TransformClient(transport: transport, config: config)

        await assertThrows(client: client, expected: .rateLimited)
    }

    func test504MapsToProviderTimeoutRegardlessOfBody() async {
        let transport = StubHTTPTransport.success(status: 504, body: Data())
        let client = TransformClient(transport: transport, config: config)

        await assertThrows(client: client, expected: .providerTimeout)
    }

    func test500MapsToProviderRejectedFromTheServerErrorBody() async {
        let transport = StubHTTPTransport.success(status: 500, body: errorBody(code: "INTERNAL_ERROR"))
        let client = TransformClient(transport: transport, config: config)

        await assertThrows(client: client, expected: .providerRejected)
    }

    func testUnparseableErrorBodyFallsBackToProviderRejected() async {
        let transport = StubHTTPTransport.success(status: 500, body: "not-json".data(using: .utf8)!)
        let client = TransformClient(transport: transport, config: config)

        await assertThrows(client: client, expected: .providerRejected)
    }

    func testUnparseable200BodyIsInvalidResponse() async {
        let transport = StubHTTPTransport.success(status: 200, body: "not-json".data(using: .utf8)!)
        let client = TransformClient(transport: transport, config: config)

        await assertThrows(client: client, expected: .invalidResponse)
    }

    func testResponseWithDivergentRequestIdIsInvalidResponse() async {
        let mismatched = TransformResponse(
            requestId: UUID(),
            sourceLanguage: "English",
            targetLanguage: "Portuguese",
            result: "Resultado"
        )
        let transport = StubHTTPTransport.success(status: 200, body: try! JSONEncoder().encode(mismatched))
        let client = TransformClient(transport: transport, config: config)

        await assertThrows(client: client, expected: .invalidResponse)
    }

    func testTransportFailureIsWrappedAsTransportError() async {
        let transport = StubHTTPTransport.failure(StubHTTPTransportError())
        let client = TransformClient(transport: transport, config: config)

        do {
            _ = try await client.transform(makeRequest(), accessToken: "token")
            XCTFail("expected transport error")
        } catch VerbalixError.transport {
            // expected
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    func testInvalidLocalRequestNeverReachesTheTransport() async {
        let transport = StubHTTPTransport.success(status: 200, body: responseBody())
        let client = TransformClient(transport: transport, config: config)
        let invalidRequest = TransformRequest(requestId: requestId, operation: .translate, text: "")

        await assertThrows(client: client, request: invalidRequest, expected: .invalidResponse)
        XCTAssertTrue(transport.capturedRequests.isEmpty)
    }

    private func assertThrows(
        client: TransformClient,
        request: TransformRequest? = nil,
        expected: VerbalixError,
        file: StaticString = #filePath,
        line: UInt = #line
    ) async {
        do {
            _ = try await client.transform(request ?? makeRequest(), accessToken: "token")
            XCTFail("expected \(expected)", file: file, line: line)
        } catch let error as VerbalixError {
            XCTAssertEqual(error, expected, file: file, line: line)
        } catch {
            XCTFail("unexpected error type: \(error)", file: file, line: line)
        }
    }
}
