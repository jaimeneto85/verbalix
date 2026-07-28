import SwiftUI
import UIKit
import VerbalixKit

struct EditorView: View {
    @Environment(AppSession.self) private var session

    @State private var text = ""
    @State private var transforming = false
    @State private var errorMessage: String? = nil

    var body: some View {
        NavigationStack {
            ZStack(alignment: .topLeading) {
                TransformableTextEditor(
                    text: $text,
                    isTransforming: $transforming,
                    errorMessage: $errorMessage,
                    onTransform: performTransform
                )
                .ignoresSafeArea(.keyboard)

                if text.isEmpty {
                    Text("Cole ou escreva texto aqui para traduzir ou aprimorar…")
                        .foregroundStyle(.tertiary)
                        .padding(.horizontal, 16)
                        .padding(.top, 8)
                        .allowsHitTesting(false)
                }
            }
            .navigationTitle("Editor")
            .overlay(alignment: .bottom) {
                if transforming {
                    HStack(spacing: 8) {
                        ProgressView()
                        Text("Transformando…")
                            .font(.footnote)
                    }
                    .padding(12)
                    .background(.regularMaterial, in: .rect(cornerRadius: 12))
                    .padding(.bottom, 8)
                }
            }
            .overlay(alignment: .bottom) {
                if let error = errorMessage {
                    Text(error)
                        .font(.footnote)
                        .foregroundStyle(.white)
                        .padding(12)
                        .background(Color.red, in: .rect(cornerRadius: 12))
                        .padding(.bottom, 8)
                        .onTapGesture { errorMessage = nil }
                }
            }
        }
    }

    private func performTransform(
        operation: TransformOperation,
        range: NSRange,
        in textView: UITextView
    ) {
        guard let token = session.accessToken else {
            errorMessage = "Faça login para transformar texto."
            return
        }
        guard let config = BackendConfig(infoPlist: Bundle.main.infoDictionary ?? [:]) else {
            errorMessage = "Configuração indisponível."
            return
        }
        guard let swiftRange = Range(range, in: textView.text) else { return }
        let selected = String(textView.text[swiftRange])
        guard !selected.isEmpty else { return }

        transforming = true
        errorMessage = nil

        Task {
            defer { transforming = false }
            do {
                let prefs = TransformPreferences(formality: 3, length: .balanced, tone: .technical)
                let request = TransformRequest(
                    operation: operation,
                    text: selected,
                    preferences: operation == .improve ? prefs : nil
                )
                let client = TransformClient(config: config)
                let response = try await client.transform(request, accessToken: token)

                await MainActor.run {
                    textView.selectedRange = range
                    textView.replace(
                        textView.selectedTextRange ?? UITextRange(),
                        withText: response.result
                    )
                    text = textView.text
                }
            } catch let error as VerbalixError {
                errorMessage = ErrorMessages.message(for: error)
            } catch {
                errorMessage = "Erro de conexão."
            }
        }
    }
}

private struct TransformableTextEditor: UIViewRepresentable {
    @Binding var text: String
    @Binding var isTransforming: Bool
    @Binding var errorMessage: String?
    let onTransform: (TransformOperation, NSRange, UITextView) -> Void

    func makeUIView(context: Context) -> UITextView {
        let textView = UITextView()
        textView.font = .preferredFont(forTextStyle: .body)
        textView.delegate = context.coordinator
        textView.isEditable = true
        textView.isScrollEnabled = true
        textView.textContainerInset = UIEdgeInsets(top: 8, left: 12, bottom: 8, right: 12)
        textView.backgroundColor = .systemBackground

        let interaction = UIEditMenuInteraction(delegate: context.coordinator)
        textView.addInteraction(interaction)
        context.coordinator.interaction = interaction

        return textView
    }

    func updateUIView(_ textView: UITextView, context: Context) {
        if textView.text != text {
            textView.text = text
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(binding: $text, onTransform: onTransform)
    }

    final class Coordinator: NSObject, UITextViewDelegate, UIEditMenuInteractionDelegate {
        private let binding: Binding<String>
        private let onTransform: (TransformOperation, NSRange, UITextView) -> Void
        var interaction: UIEditMenuInteraction?
        private weak var textView: UITextView?

        init(binding: Binding<String>, onTransform: @escaping (TransformOperation, NSRange, UITextView) -> Void) {
            self.binding = binding
            self.onTransform = onTransform
        }

        func textViewDidChange(_ textView: UITextView) {
            binding.wrappedValue = textView.text
            self.textView = textView
        }

        func textViewDidChangeSelection(_ textView: UITextView) {
            self.textView = textView
            let range = textView.selectedRange
            guard range.length > 0 else { return }

            let location = CGPoint(
                x: textView.center.x,
                y: textView.center.y
            )
            let config = UIEditMenuConfiguration(identifier: nil, sourcePoint: location)
            interaction?.presentEditMenu(with: config)
        }

        func editMenuInteraction(
            _ interaction: UIEditMenuInteraction,
            menuFor configuration: UIEditMenuConfiguration,
            suggestedActions: [UIMenuElement]
        ) -> UIMenu? {
            guard let tv = textView, tv.selectedRange.length > 0 else { return nil }
            let range = tv.selectedRange

            let translate = UIAction(title: "Traduzir") { [weak self] _ in
                self?.onTransform(.translate, range, tv)
            }
            let improve = UIAction(title: "Aprimorar") { [weak self] _ in
                self?.onTransform(.improve, range, tv)
            }
            return UIMenu(children: [translate, improve])
        }
    }
}
