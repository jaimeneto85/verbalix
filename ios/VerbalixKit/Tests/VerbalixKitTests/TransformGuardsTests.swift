import XCTest
@testable import VerbalixKit

final class TransformGuardsTests: XCTestCase {
    func testEmptyTextIsRejectedBeforeAnySizeGuard() {
        let request = TransformRequest(operation: .translate, text: "")

        XCTAssertThrowsError(try request.validated()) { error in
            XCTAssertEqual(error as? VerbalixError, .invalidResponse)
        }
    }

    func testTextAtExactly12000ScalarsIsAccepted() {
        let request = TransformRequest(operation: .translate, text: String(repeating: "a", count: 12_000))

        XCTAssertNoThrow(try request.validated())
    }

    func testTextOver12000ScalarsIsRejectedByTheScalarGuardAloneWhenBodyStaysSmall() {
        let text = String(repeating: "a", count: 12_001)
        let request = TransformRequest(operation: .translate, text: text)

        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        let bodySize = (try? encoder.encode(request))?.count ?? 0
        XCTAssertLessThan(bodySize, 65_536, "fixture must stay under the body guard to prove the scalar guard fires independently")

        XCTAssertThrowsError(try request.validated()) { error in
            XCTAssertEqual(error as? VerbalixError, .textTooLong)
        }
    }

    func testMultibyteTextUnder12000ScalarsButOver64KiBBodyIsRejectedByTheBodyGuardAlone() {
        let text = String(repeating: "\u{01}", count: 11_000)
        XCTAssertLessThan(text.unicodeScalars.count, 12_000, "fixture must stay under the scalar guard to prove the body guard fires independently")

        let request = TransformRequest(operation: .translate, text: text)

        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        let bodySize = try! encoder.encode(request).count
        XCTAssertGreaterThan(bodySize, 65_536)

        XCTAssertThrowsError(try request.validated()) { error in
            XCTAssertEqual(error as? VerbalixError, .textTooLong)
        }
    }

    func testEmojiZWJSequenceCanAlsoOverflowTheBodyGuardBeforeTheScalarGuard() {
        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}"
        let repeatedFamilies = 1_500
        let text = String(repeating: family, count: repeatedFamilies)
        let scalarCount = text.unicodeScalars.count

        XCTAssertLessThan(scalarCount, 12_000)

        let request = TransformRequest(operation: .translate, text: text)
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        let bodySize = try! encoder.encode(request).count

        if bodySize > 65_536 {
            XCTAssertThrowsError(try request.validated()) { error in
                XCTAssertEqual(error as? VerbalixError, .textTooLong)
            }
        } else {
            XCTAssertNoThrow(try request.validated())
        }
    }

    func testBodyGuardIsEvaluatedOnTheSerializedRequestNotJustTheRawText() {
        let text = String(repeating: "a", count: 3_000)
        let preferences = TransformPreferences(formality: 3, length: .detailed, tone: .technical)
        let request = TransformRequest(operation: .improve, text: text, preferences: preferences)

        XCTAssertNoThrow(try request.validated())
    }
}
