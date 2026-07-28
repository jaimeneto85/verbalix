import XCTest
@testable import VerbalixKit

final class TransformContractTests: XCTestCase {
    // Fixture literal shared with supabase/functions/transform/contract_test.ts
    private let fixtureRequestId = UUID(uuidString: "b65c8888-fb0e-4a8f-9fee-95268995bf68")!
    private let secondFixtureRequestId = UUID(uuidString: "ae63a86a-b5a2-4893-aea3-d78653385ed9")!

    func testAcceptsABoundedTranslationRequest() throws {
        let request = TransformRequest(
            requestId: fixtureRequestId,
            operation: .translate,
            text: "Corrija o handler `onSubmit`."
        )

        XCTAssertNoThrow(try request.validated())
    }

    func testImprovePreferencesAreMandatory() {
        let request = TransformRequest(
            requestId: fixtureRequestId,
            operation: .improve,
            text: "Improve this"
        )

        XCTAssertThrowsError(try request.validated()) { error in
            XCTAssertEqual(error as? VerbalixError, .invalidResponse)
        }
    }

    func testTranslateWithoutPreferencesValidatesFine() {
        let request = TransformRequest(
            requestId: fixtureRequestId,
            operation: .translate,
            text: "texto"
        )

        XCTAssertNoThrow(try request.validated())
    }

    func testTranslateIgnoresOptionalPreferencesWhenFormalityIsValid() {
        let request = TransformRequest(
            requestId: fixtureRequestId,
            operation: .translate,
            text: "texto",
            preferences: TransformPreferences(formality: 3, length: .balanced, tone: .technical)
        )

        XCTAssertNoThrow(try request.validated())
    }

    func testImproveWithValidPreferencesValidates() throws {
        let request = TransformRequest(
            requestId: secondFixtureRequestId,
            operation: .improve,
            text: "Improve this",
            preferences: TransformPreferences(formality: 4, length: .concise, tone: .technical)
        )

        XCTAssertNoThrow(try request.validated())
    }

    func testRejectsFractionalFormalityBoundary() {
        for formality in [0, 6] {
            let request = TransformRequest(
                requestId: fixtureRequestId,
                operation: .improve,
                text: "Improve this",
                preferences: TransformPreferences(formality: formality, length: .balanced, tone: .technical)
            )

            XCTAssertThrowsError(try request.validated()) { error in
                XCTAssertEqual(error as? VerbalixError, .invalidResponse)
            }
        }
    }

    func testTranslateResponseRequiresNonEmptyTargetLanguage() {
        let request = try! TransformRequest(
            requestId: fixtureRequestId,
            operation: .translate,
            text: "Translate this"
        ).validated()

        let missingTarget = TransformResponse(
            requestId: fixtureRequestId,
            sourceLanguage: "English",
            targetLanguage: nil,
            result: "Resultado"
        )
        XCTAssertThrowsError(try missingTarget.validated(for: request)) { error in
            XCTAssertEqual(error as? VerbalixError, .invalidResponse)
        }

        let validTarget = TransformResponse(
            requestId: fixtureRequestId,
            sourceLanguage: "English",
            targetLanguage: "Portuguese",
            result: "Resultado"
        )
        XCTAssertNoThrow(try validTarget.validated(for: request))
    }

    func testImproveResponseRejectsNonNilTargetLanguage() throws {
        let request = try TransformRequest(
            requestId: secondFixtureRequestId,
            operation: .improve,
            text: "Improve this",
            preferences: TransformPreferences(formality: 3, length: .balanced, tone: .technical)
        ).validated()

        let response = TransformResponse(
            requestId: secondFixtureRequestId,
            sourceLanguage: "English",
            targetLanguage: "Portuguese",
            result: "Improved"
        )

        XCTAssertThrowsError(try response.validated(for: request)) { error in
            XCTAssertEqual(error as? VerbalixError, .invalidResponse)
        }
    }

    func testImproveResponseAcceptsNilTargetLanguage() throws {
        let request = try TransformRequest(
            requestId: secondFixtureRequestId,
            operation: .improve,
            text: "Improve this",
            preferences: TransformPreferences(formality: 3, length: .balanced, tone: .technical)
        ).validated()

        let response = TransformResponse(
            requestId: secondFixtureRequestId,
            sourceLanguage: "English",
            targetLanguage: nil,
            result: "Improved"
        )

        XCTAssertNoThrow(try response.validated(for: request))
    }

    func testResultOverCharacterLimitIsRejected() throws {
        let request = try TransformRequest(
            requestId: fixtureRequestId,
            operation: .translate,
            text: "Translate this"
        ).validated()

        let response = TransformResponse(
            requestId: fixtureRequestId,
            sourceLanguage: "English",
            targetLanguage: "Portuguese",
            result: String(repeating: "界", count: 24_001)
        )

        XCTAssertThrowsError(try response.validated(for: request)) { error in
            XCTAssertEqual(error as? VerbalixError, .invalidResponse)
        }
    }

    func testResultExactlyAtCharacterLimitIsAccepted() throws {
        let request = try TransformRequest(
            requestId: fixtureRequestId,
            operation: .translate,
            text: "Translate this"
        ).validated()

        let response = TransformResponse(
            requestId: fixtureRequestId,
            sourceLanguage: "English",
            targetLanguage: "Portuguese",
            result: String(repeating: "界", count: 24_000)
        )

        XCTAssertNoThrow(try response.validated(for: request))
    }

    func testDivergentRequestIdIsRejected() throws {
        let request = try TransformRequest(
            requestId: fixtureRequestId,
            operation: .translate,
            text: "Translate this"
        ).validated()

        let response = TransformResponse(
            requestId: secondFixtureRequestId,
            sourceLanguage: "English",
            targetLanguage: "Portuguese",
            result: "Resultado"
        )

        XCTAssertThrowsError(try response.validated(for: request)) { error in
            XCTAssertEqual(error as? VerbalixError, .invalidResponse)
        }
    }

    func testRequestIdIsSerializedInLowercase() throws {
        let request = TransformRequest(
            requestId: fixtureRequestId,
            operation: .translate,
            text: "texto"
        )

        let data = try JSONEncoder().encode(request)
        let json = try XCTUnwrap(String(data: data, encoding: .utf8))

        XCTAssertTrue(
            json.contains(fixtureRequestId.uuidString.lowercased()),
            "requestId must be serialized in lowercase to mirror contract.ts's isUuid()/randomUUID() convention"
        )
    }

    func testRoundTripDecodesRequestFromServerShapedJSON() throws {
        let json = """
        {"requestId":"\(fixtureRequestId.uuidString.lowercased())","operation":"improve","text":"Improve this","preferences":{"formality":3,"length":"balanced","tone":"technical"}}
        """.data(using: .utf8)!

        let decoded = try JSONDecoder().decode(TransformRequest.self, from: json)

        XCTAssertEqual(decoded.requestId, fixtureRequestId)
        XCTAssertEqual(decoded.operation, .improve)
        XCTAssertEqual(decoded.preferences?.formality, 3)
        XCTAssertEqual(decoded.preferences?.length, .balanced)
        XCTAssertEqual(decoded.preferences?.tone, .technical)
    }
}
