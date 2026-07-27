import Foundation

public struct BackendConfig: Sendable {
    public let supabaseURL: URL
    public let anonKey: String
    public let authCallbackURL: URL

    public init?(
        supabaseURL: URL,
        anonKey: String,
        authCallback: URL = URL(string: "verbalix-ios://auth/callback")!
    ) {
        guard !anonKey.trimmingCharacters(in: .whitespaces).isEmpty else { return nil }
        self.supabaseURL = supabaseURL
        self.anonKey = anonKey
        self.authCallbackURL = authCallback
    }

    public init?(infoPlist: [String: Any]) {
        guard
            let urlString = infoPlist["VerbalixSupabaseURL"] as? String,
            let url = URL(string: urlString),
            let key = infoPlist["VerbalixSupabaseAnonKey"] as? String,
            !key.trimmingCharacters(in: .whitespaces).isEmpty
        else { return nil }

        self.supabaseURL = url
        self.anonKey = key

        if let callbackString = infoPlist["VerbalixAuthCallback"] as? String,
           !callbackString.trimmingCharacters(in: .whitespaces).isEmpty,
           let callbackURL = URL(string: callbackString) {
            self.authCallbackURL = callbackURL
        } else {
            self.authCallbackURL = URL(string: "verbalix-ios://auth/callback")!
        }
    }
}

extension BackendConfig {
    public var transformEndpoint: URL {
        supabaseURL.appendingPathComponent("functions/v1/transform")
    }

    public var historyEndpoint: URL {
        supabaseURL.appendingPathComponent("rest/v1/transform_history")
    }

    public var preferencesEndpoint: URL {
        supabaseURL.appendingPathComponent("rest/v1/user_preferences")
    }

    public var userEndpoint: URL {
        supabaseURL.appendingPathComponent("auth/v1/user")
    }

    public var authTokenEndpoint: URL {
        supabaseURL
            .appendingPathComponent("auth")
            .appendingPathComponent("v1")
            .appendingPathComponent("token")
    }
}
