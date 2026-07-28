import Auth
import Foundation

public enum AuthCallbackOutcome {
    case proceed(URL)
    case failure(VerbalixError)
}

public enum AuthCallback {

    private struct AllowedEndpoint {
        let scheme: String
        let host: String
        let path: String
    }

    private static let allowedEndpoints: [AllowedEndpoint] = [
        AllowedEndpoint(scheme: "https", host: "app.verbali.xyz", path: "/auth/callback"),
        AllowedEndpoint(scheme: "verbalix-ios", host: "auth", path: "/callback"),
    ]

    public static func parse(_ url: URL) -> AuthCallbackOutcome {
        guard isAllowedURL(url) else {
            return .failure(.authLinkExpired)
        }

        let params = extractParams(from: url)

        if let errorCode = params["error_code"] {
            return .failure(classifyErrorCode(errorCode))
        }

        if params["error"] != nil || params["error_description"] != nil {
            return .failure(.authLinkExpired)
        }

        guard params["code"] != nil else {
            return .failure(.authLinkExpired)
        }

        return .proceed(url)
    }

    static func classifyPKCEError(_ authError: AuthError) -> VerbalixError {
        switch authError {
        case .pkceGrantCodeExchange(_, _, let code) where code == "otp_expired":
            return .authLinkExpired
        case .pkceGrantCodeExchange:
            return .authLinkAlreadyUsed
        default:
            return .authLinkExpired
        }
    }

    private static func isAllowedURL(_ url: URL) -> Bool {
        guard let scheme = url.scheme, let host = url.host else { return false }
        return allowedEndpoints.contains { $0.scheme == scheme && $0.host == host && url.path == $0.path }
    }

    private static func extractParams(from url: URL) -> [String: String] {
        guard let components = URLComponents(url: url, resolvingAgainstBaseURL: false) else {
            return [:]
        }

        var result: [String: String] = [:]

        if let fragment = components.percentEncodedFragment {
            for (name, value) in parseFormEncodedPairs(fragment) {
                result[name] = value
            }
        }

        if let query = components.percentEncodedQuery {
            for (name, value) in parseFormEncodedPairs(query) {
                result[name] = value
            }
        }

        return result
    }

    private static func parseFormEncodedPairs(_ percentEncodedString: String) -> [(String, String)] {
        percentEncodedString
            .split(separator: "&")
            .compactMap { pair in
                let parts = pair.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
                guard parts.count == 2, !parts[1].isEmpty else { return nil }
                return (decodeFormComponent(parts[0]), decodeFormComponent(parts[1]))
            }
    }

    private static func decodeFormComponent(_ component: Substring) -> String {
        let plusDecoded = component.replacingOccurrences(of: "+", with: " ")
        return plusDecoded.removingPercentEncoding ?? plusDecoded
    }

    private static func classifyErrorCode(_ code: String) -> VerbalixError {
        switch code {
        case "otp_expired":
            return .authLinkExpired
        default:
            return .authLinkExpired
        }
    }
}
