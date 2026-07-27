import Auth
import Foundation

public final class AuthService: @unchecked Sendable {
    public static let callbackURL = URL(string: "https://app.verbali.xyz/auth/callback")!

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
        switch AuthCallback.parse(url) {
        case .failure(let error):
            throw error
        case .proceed(let callbackURL):
            do {
                let session = try await client.session(from: callbackURL)
                return StoredSession(accessToken: session.accessToken, refreshToken: session.refreshToken)
            } catch let authError as AuthError {
                throw AuthCallback.classifyPKCEError(authError)
            } catch {
                throw VerbalixError.transport(error)
            }
        }
    }

    public func currentSession() async -> StoredSession? {
        guard let session = try? await client.session else { return nil }
        return StoredSession(accessToken: session.accessToken, refreshToken: session.refreshToken)
    }

    public func signOut() async throws {
        try await client.signOut()
    }
}
