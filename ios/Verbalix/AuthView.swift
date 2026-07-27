import SwiftUI
import VerbalixKit

private enum AuthState: Equatable {
    case idle
    case sending
    case sent
    case error(String)
}

struct AuthView: View {
    @Environment(AppSession.self) private var session

    @State private var email = ""
    @State private var state: AuthState = .idle

    var body: some View {
        NavigationStack {
            VStack(spacing: 32) {
                headerSection

                if let callbackError = session.callbackError {
                    Text(callbackError)
                        .font(.footnote)
                        .foregroundStyle(.red)
                        .padding(12)
                        .background(Color.red.opacity(0.1), in: .rect(cornerRadius: 8))
                        .onTapGesture { session.clearCallbackError() }
                }

                switch state {
                case .idle, .error:
                    inputSection
                case .sending:
                    sendingSection
                case .sent:
                    sentSection
                }

                Spacer()
            }
            .padding(24)
            .navigationTitle("Entrar")
            .navigationBarTitleDisplayMode(.large)
        }
    }

    private var headerSection: some View {
        VStack(spacing: 16) {
            Image(systemName: "text.bubble.fill")
                .font(.system(size: 64))
                .foregroundStyle(.tint)

            Text("Verbalix")
                .font(.largeTitle.bold())

            Text("Transforme texto com IA diretamente no seu iPhone.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
    }

    private var inputSection: some View {
        VStack(spacing: 16) {
            if case .error(let message) = state {
                Text(message)
                    .font(.footnote)
                    .foregroundStyle(.red)
                    .padding(12)
                    .background(Color.red.opacity(0.1), in: .rect(cornerRadius: 8))
            }

            TextField("seu@email.com", text: $email)
                .textFieldStyle(.roundedBorder)
                .keyboardType(.emailAddress)
                .textContentType(.emailAddress)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)

            Button(action: sendMagicLink) {
                Text("Enviar link de acesso")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .disabled(email.trimmingCharacters(in: .whitespaces).isEmpty)
        }
    }

    private var sendingSection: some View {
        VStack(spacing: 16) {
            ProgressView()
                .controlSize(.large)
            Text("Enviando link…")
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity)
    }

    private var sentSection: some View {
        VStack(spacing: 16) {
            Image(systemName: "envelope.circle.fill")
                .font(.system(size: 56))
                .foregroundStyle(.tint)

            Text("Verifique seu e-mail")
                .font(.title2.bold())

            Text("Enviamos um link para **\(email)**. Toque no link para entrar.")
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)

            Button("Usar outro e-mail") {
                state = .idle
                email = ""
            }
            .font(.footnote)
        }
    }

    private func sendMagicLink() {
        let trimmed = email.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return }
        state = .sending
        Task {
            do {
                try await session.authService.sendMagicLink(email: trimmed)
                state = .sent
            } catch let error as VerbalixError {
                state = .error(ErrorMessages.message(for: error))
            } catch {
                state = .error("Não foi possível enviar o link. Tente novamente.")
            }
        }
    }
}
