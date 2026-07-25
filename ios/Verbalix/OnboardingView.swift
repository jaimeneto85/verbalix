import SwiftUI

struct OnboardingView: View {
    var body: some View {
        NavigationStack {
            List {
                Section {
                    OnboardingStepView(
                        number: "1",
                        title: "Abra os Ajustes do iPhone",
                        description: "Vá em Ajustes → Geral → Teclado → Teclados → Adicionar novo teclado.",
                        systemImage: "gear"
                    )
                    OnboardingStepView(
                        number: "2",
                        title: "Adicione o teclado Verbalix",
                        description: "Encontre \"Verbalix\" na lista e toque para adicionar.",
                        systemImage: "keyboard"
                    )
                    OnboardingStepView(
                        number: "3",
                        title: "Conceda Acesso Total",
                        description: "Toque em \"Verbalix\" na lista de teclados e ative \"Permitir Acesso Total\". Isso é necessário para que o teclado consulte a IA.",
                        systemImage: "lock.open"
                    )
                } header: {
                    Text("Configurar o teclado Verbalix")
                }

                Section {
                    Button(action: openSettings) {
                        Label("Abrir Ajustes", systemImage: "arrow.up.right")
                    }
                } header: {
                    Text("Atalho")
                } footer: {
                    Text("Após adicionar o teclado, troque-o em qualquer campo de texto segurando o ícone do globo.")
                }

                Section {
                    OnboardingStepView(
                        number: "1",
                        title: "Selecione texto em qualquer app",
                        description: "Selecione o texto que deseja transformar.",
                        systemImage: "selection.pin.in.out"
                    )
                    OnboardingStepView(
                        number: "2",
                        title: "Toque em Compartilhar → Verbalix",
                        description: "Use o menu de compartilhamento ou a extensão de ação para transformar o texto selecionado.",
                        systemImage: "square.and.arrow.up"
                    )
                } header: {
                    Text("Usar a extensão de Ação")
                }
            }
            .navigationTitle("Configuração")
            .listStyle(.insetGrouped)
        }
    }

    private func openSettings() {
        guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
        UIApplication.shared.open(url)
    }
}

private struct OnboardingStepView: View {
    let number: String
    let title: String
    let description: String
    let systemImage: String

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            ZStack {
                Circle()
                    .fill(Color.accentColor)
                    .frame(width: 28, height: 28)
                Text(number)
                    .font(.system(size: 13, weight: .bold))
                    .foregroundStyle(.white)
            }

            VStack(alignment: .leading, spacing: 4) {
                Text(title)
                    .font(.subheadline.weight(.semibold))
                Text(description)
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.vertical, 4)
    }
}
