import SwiftUI
import VerbalixKit

@main
struct VerbalixApp: App {
    @State private var appSession: AppSession? = nil
    @State private var pendingURL: URL? = nil

    var body: some Scene {
        WindowGroup {
            Group {
                if let session = appSession {
                    RootView()
                        .environment(session)
                } else {
                    ProgressView()
                }
            }
            .onOpenURL { url in
                if let session = appSession {
                    Task { await session.handleDeepLink(url) }
                } else {
                    pendingURL = url
                }
            }
            .task {
                guard let config = BackendConfig(infoPlist: Bundle.main.infoDictionary ?? [:]) else {
                    appSession = AppSession(config: BackendConfig(
                        supabaseURL: URL(string: "https://placeholder.supabase.co")!,
                        anonKey: "placeholder"
                    )!)
                    return
                }
                let session = AppSession(config: config)
                appSession = session
                await session.checkSession()
                if let url = pendingURL {
                    pendingURL = nil
                    await session.handleDeepLink(url)
                }
            }
        }
    }
}
