import SwiftUI

struct AuthView: View {
    let onAuthenticated: () -> Void

    var body: some View {
        VStack(spacing: 24) {
            Image(systemName: "text.bubble")
                .font(.system(size: 64))
                .foregroundStyle(.tint)

            Text("Verbalix")
                .font(.largeTitle.bold())

            Button("Entrar com magic link") {
                onAuthenticated()
            }
            .buttonStyle(.borderedProminent)
        }
        .padding()
    }
}
