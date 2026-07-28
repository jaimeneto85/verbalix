import XCTest
@testable import VerbalixKit

final class BackendConfigAuthCallbackTests: XCTestCase {

    private let validPlist: [String: Any] = [
        "VerbalixSupabaseURL": "https://example.supabase.co",
        "VerbalixSupabaseAnonKey": "some-anon-key",
    ]

    func testDefaultIsCustomSchemeWhenKeyAbsent() {
        let config = BackendConfig(infoPlist: validPlist)
        XCTAssertNotNil(config)
        XCTAssertEqual(config?.authCallbackURL, URL(string: "verbalix-ios://auth/callback")!)
    }

    func testValidHTTPSCallbackKeyIsRespected() {
        var plist = validPlist
        plist["VerbalixAuthCallback"] = "https://app.verbali.xyz/auth/callback"
        let config = BackendConfig(infoPlist: plist)
        XCTAssertNotNil(config)
        XCTAssertEqual(config?.authCallbackURL, URL(string: "https://app.verbali.xyz/auth/callback")!)
    }

    func testEmptyCallbackKeyFallsBackToDefault() {
        var plist = validPlist
        plist["VerbalixAuthCallback"] = ""
        let config = BackendConfig(infoPlist: plist)
        XCTAssertNotNil(config)
        XCTAssertEqual(config?.authCallbackURL, URL(string: "verbalix-ios://auth/callback")!)
    }

    func testWhitespaceOnlyCallbackKeyFallsBackToDefault() {
        var plist = validPlist
        plist["VerbalixAuthCallback"] = "   "
        let config = BackendConfig(infoPlist: plist)
        XCTAssertNotNil(config)
        XCTAssertEqual(config?.authCallbackURL, URL(string: "verbalix-ios://auth/callback")!)
    }

    func testInitWithExplicitCallbackURL() {
        let customURL = URL(string: "https://app.verbali.xyz/auth/callback")!
        let supabase = URL(string: "https://example.supabase.co")!
        let config = BackendConfig(supabaseURL: supabase, anonKey: "key", authCallback: customURL)
        XCTAssertNotNil(config)
        XCTAssertEqual(config?.authCallbackURL, customURL)
    }

    func testInitDefaultCallbackIsCustomScheme() {
        let supabase = URL(string: "https://example.supabase.co")!
        let config = BackendConfig(supabaseURL: supabase, anonKey: "key")
        XCTAssertNotNil(config)
        XCTAssertEqual(config?.authCallbackURL, URL(string: "verbalix-ios://auth/callback")!)
    }
}

final class AuthCallbackRegressionTests: XCTestCase {

    func testParseBothFormsAccepted() {
        let httpsURL = URL(string: "https://app.verbali.xyz/auth/callback?code=abc")!
        let customURL = URL(string: "verbalix-ios://auth/callback?code=abc")!

        guard case .proceed = AuthCallback.parse(httpsURL) else {
            XCTFail("Expected .proceed for https form")
            return
        }
        guard case .proceed = AuthCallback.parse(customURL) else {
            XCTFail("Expected .proceed for custom scheme form")
            return
        }
    }
}
