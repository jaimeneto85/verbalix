import Foundation

public struct BackendConfig: Sendable {
    public let supabaseURL: URL
    public let anonKey: String

    public init?(supabaseURL: URL, anonKey: String) {
        guard !anonKey.trimmingCharacters(in: .whitespaces).isEmpty else { return nil }
        self.supabaseURL = supabaseURL
        self.anonKey = anonKey
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
