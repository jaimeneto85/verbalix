import SwiftUI
import VerbalixKit

struct RootView: View {
    @Environment(AppSession.self) private var session

    var body: some View {
        Group {
            if session.isLoading {
                ProgressView()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if session.accessToken != nil {
                MainTabView()
            } else {
                AuthView()
            }
        }
        .animation(.easeInOut, value: session.accessToken)
    }
}
