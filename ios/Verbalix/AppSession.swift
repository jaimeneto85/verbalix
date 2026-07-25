import Foundation
import Observation
import VerbalixKit

@MainActor
@Observable
final class AppSession {
    private(set) var accessToken: String? = nil
    private(set) var isLoading: Bool = true

    let authService: AuthService

    private let sessionStore: SharedSessionStore

    init(config: BackendConfig) {
        sessionStore = SharedSessionStore(
            service: "com.verbalix.session",
            accessGroup: "com.verbalix.shared"
        )
        let storage = VerbalixAuthStorage(
            sessionStore: sessionStore,
            service: "com.verbalix.session",
            accessGroup: "com.verbalix.shared"
        )
        authService = AuthService(config: config, storage: storage)
    }

    func checkSession() async {
        isLoading = true
        defer { isLoading = false }
        let stored = await authService.currentSession()
        accessToken = stored?.accessToken
    }

    func handleDeepLink(_ url: URL) async {
        do {
            let stored = try await authService.handleDeepLink(url)
            accessToken = stored.accessToken
        } catch {}
    }

    func signOut() async {
        try? await authService.signOut()
        accessToken = nil
    }
}
