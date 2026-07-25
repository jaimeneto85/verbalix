import SwiftUI
import VerbalixKit

@main
struct VerbalixApp: App {
    @State private var appSession: AppSession? = nil

    var body: some Scene {
        WindowGroup {
            Group {
                if let session = appSession {
                    RootView()
                        .environment(session)
                        .onOpenURL { url in
                            Task { await session.handleDeepLink(url) }
                        }
                } else {
                    ProgressView()
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
            }
        }
    }
}
