import Foundation
import Observation
import VerbalixKit

@MainActor
@Observable
final class AppSession {
    private(set) var accessToken: String? = nil
    private(set) var isLoading: Bool = true
    private(set) var callbackError: String? = nil

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
        callbackError = nil
        do {
            let stored = try await authService.handleDeepLink(url)
            accessToken = stored.accessToken
        } catch let error as VerbalixError {
            callbackError = ErrorMessages.message(for: error)
        } catch {
            callbackError = "Não foi possível processar o link de acesso. Tente novamente."
        }
    }

    func clearCallbackError() {
        callbackError = nil
    }

    func signOut() async {
        try? await authService.signOut()
        accessToken = nil
    }
}
