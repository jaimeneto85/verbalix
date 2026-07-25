import SwiftUI
import VerbalixKit

struct RootView: View {
    @State private var isAuthenticated = false

    var body: some View {
        if isAuthenticated {
            MainTabView()
        } else {
            AuthView(onAuthenticated: { isAuthenticated = true })
        }
    }
}
