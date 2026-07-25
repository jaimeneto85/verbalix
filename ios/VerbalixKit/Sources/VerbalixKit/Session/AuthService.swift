import Auth
import Foundation

public final class AuthService: @unchecked Sendable {
    public static let callbackURL = URL(string: "verbalix-ios://auth/callback")!

    private let client: AuthClient

    public init(config: BackendConfig, storage: VerbalixAuthStorage) {
        client = AuthClient(
            url: config.supabaseURL.appendingPathComponent("auth/v1"),
            headers: ["apikey": config.anonKey],
            flowType: .pkce,
            localStorage: storage,
            logger: nil
        )
    }

    public func sendMagicLink(email: String) async throws {
        try await client.signInWithOTP(
            email: email,
            redirectTo: Self.callbackURL
        )
    }

    public func handleDeepLink(_ url: URL) async throws -> StoredSession {
        let session = try await client.session(from: url)
        return StoredSession(accessToken: session.accessToken, refreshToken: session.refreshToken)
    }

    public func currentSession() async -> StoredSession? {
        guard let session = try? await client.session else { return nil }
        return StoredSession(accessToken: session.accessToken, refreshToken: session.refreshToken)
    }

    public func signOut() async throws {
        try await client.signOut()
    }
}
