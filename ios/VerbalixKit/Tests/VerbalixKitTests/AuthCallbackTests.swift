import Auth
import XCTest
@testable import VerbalixKit

final class AuthCallbackTests: XCTestCase {

    func testEC1_validHTTPSURL_returnsProceed() {
        let url = URL(string: "https://app.verbali.xyz/auth/callback?code=abc123")!
        let outcome = AuthCallback.parse(url)
        guard case .proceed(let proceededURL) = outcome else {
            XCTFail("Expected .proceed, got \(outcome)")
            return
        }
        XCTAssertEqual(proceededURL, url)
    }

    func testEC2_validCustomSchemeURL_returnsProceed() {
        let url = URL(string: "verbalix-ios://auth/callback?code=abc123")!
        let outcome = AuthCallback.parse(url)
        guard case .proceed(let proceededURL) = outcome else {
            XCTFail("Expected .proceed, got \(outcome)")
            return
        }
        XCTAssertEqual(proceededURL, url)
    }

    func testEC3_wrongHost_returnsFailure() {
        let url = URL(string: "https://evil.com/auth/callback?code=x")!
        let outcome = AuthCallback.parse(url)
        guard case .failure = outcome else {
            XCTFail("Expected .failure, got \(outcome)")
            return
        }
    }

    func testEC4_wrongPath_returnsFailure() {
        let url = URL(string: "https://app.verbali.xyz/wrong?code=x")!
        let outcome = AuthCallback.parse(url)
        guard case .failure = outcome else {
            XCTFail("Expected .failure, got \(outcome)")
            return
        }
    }

    func testEC5_missingCode_returnsFailure() {
        let url = URL(string: "https://app.verbali.xyz/auth/callback")!
        let outcome = AuthCallback.parse(url)
        guard case .failure = outcome else {
            XCTFail("Expected .failure, got \(outcome)")
            return
        }
    }

    func testEC6_otpExpiredError_returnsAuthLinkExpiredWithPTBRMessage() {
        let url = URL(string: "https://app.verbali.xyz/auth/callback?error=access_denied&error_code=otp_expired&error_description=Email+link+is+invalid+or+has+already+been+used")!
        let outcome = AuthCallback.parse(url)
        guard case .failure(let error) = outcome else {
            XCTFail("Expected .failure, got \(outcome)")
            return
        }
        XCTAssertEqual(error, .authLinkExpired)
        XCTAssertEqual(
            ErrorMessages.message(for: error),
            "O link de acesso expirou ou é inválido. Solicite um novo link."
        )
    }

    func testEC7_pkceGrantCodeExchangeError_returnsAuthLinkAlreadyUsedWithPTBRMessage() {
        let authError = AuthError.pkceGrantCodeExchange(
            message: "A code verifier wasn't found in PKCE flow.",
            error: "invalid_request",
            code: "invalid_grant"
        )
        let verbalixError = AuthCallback.classifyPKCEError(authError)
        XCTAssertEqual(verbalixError, .authLinkAlreadyUsed)
        XCTAssertEqual(
            ErrorMessages.message(for: verbalixError),
            "Este link de acesso já foi utilizado. Solicite um novo link."
        )
    }

    func testOtpExpiredCodeInPKCEError_returnsAuthLinkExpired() {
        let authError = AuthError.pkceGrantCodeExchange(
            message: "Error in URL with unspecified error_description.",
            error: "access_denied",
            code: "otp_expired"
        )
        let verbalixError = AuthCallback.classifyPKCEError(authError)
        XCTAssertEqual(verbalixError, .authLinkExpired)
    }

    func testCodeInFragmentIsAccepted() {
        let url = URL(string: "https://app.verbali.xyz/auth/callback#code=abc123")!
        let outcome = AuthCallback.parse(url)
        guard case .proceed = outcome else {
            XCTFail("Expected .proceed for fragment-based code, got \(outcome)")
            return
        }
    }

    func testErrorInFragmentIsDetected() {
        let url = URL(string: "https://app.verbali.xyz/auth/callback#error=access_denied&error_code=otp_expired")!
        let outcome = AuthCallback.parse(url)
        guard case .failure(let error) = outcome else {
            XCTFail("Expected .failure for fragment-based error, got \(outcome)")
            return
        }
        XCTAssertEqual(error, .authLinkExpired)
    }
}
