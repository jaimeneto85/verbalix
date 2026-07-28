import SwiftUI

struct MainTabView: View {
    var body: some View {
        TabView {
            EditorView()
                .tabItem { Label("Editor", systemImage: "doc.text") }

            HistoryView()
                .tabItem { Label("Histórico", systemImage: "clock") }

            SettingsView()
                .tabItem { Label("Ajustes", systemImage: "gear") }

            OnboardingView()
                .tabItem { Label("Teclado", systemImage: "keyboard") }
        }
    }
}
