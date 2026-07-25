import Foundation

public struct SessionRefresher: Sendable {
    private let config: BackendConfig
    private let store: SharedSessionStore
    private let lock: RefreshLock

    public init(config: BackendConfig, store: SharedSessionStore, appGroupID: String) {
        self.config = config
        self.store = store
        self.lock = RefreshLock(appGroupID: appGroupID, timeoutSeconds: 12)
    }

    public func validAccessToken() async throws -> String {
        guard let session = try store.load() else {
            throw VerbalixError.unauthenticated
        }
        return session.accessToken
    }

    public func refreshAndGetToken() async throws -> String {
        try await lock.withLock {
            guard let session = try store.load() else {
                throw VerbalixError.unauthenticated
            }
            let refreshed = try await performRefresh(refreshToken: session.refreshToken)
            try store.save(refreshed)
            return refreshed.accessToken
        }
    }

    private func performRefresh(refreshToken: String) async throws -> StoredSession {
        var components = URLComponents(url: config.authTokenEndpoint, resolvingAgainstBaseURL: false)!
        components.queryItems = [URLQueryItem(name: "grant_type", value: "refresh_token")]

        var request = URLRequest(url: components.url!)
        request.httpMethod = "POST"
        request.setValue(config.anonKey, forHTTPHeaderField: "apikey")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONEncoder().encode(["refresh_token": refreshToken])
        request.timeoutInterval = 12

        let (data, response) = try await URLSession.shared.data(for: request)

        guard let http = response as? HTTPURLResponse else {
            throw VerbalixError.invalidResponse
        }

        if http.statusCode == 401 || http.statusCode == 400 {
            throw VerbalixError.unauthenticated
        }

        guard (200..<300).contains(http.statusCode) else {
            throw VerbalixError.providerRejected
        }

        struct TokenBody: Decodable {
            let access_token: String
            let refresh_token: String
        }

        guard let decoded = try? JSONDecoder().decode(TokenBody.self, from: data) else {
            throw VerbalixError.invalidResponse
        }

        return StoredSession(accessToken: decoded.access_token, refreshToken: decoded.refresh_token)
    }
}
