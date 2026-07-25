import UIKit
import UniformTypeIdentifiers
import VerbalixKit

final class ActionViewController: UIViewController {
    private enum State {
        case loading
        case result(String)
        case error(ActionError)
    }

    private enum ActionError {
        case unauthenticated
        case noText
        case textTooLong
        case timeout
        case rateLimited
        case general(String)

        var title: String {
            switch self {
            case .unauthenticated: return "Não autenticado"
            case .noText: return "Sem texto"
            case .textTooLong: return "Texto muito longo"
            case .timeout: return "Tempo esgotado"
            case .rateLimited: return "Limite atingido"
            case .general: return "Erro"
            }
        }

        var message: String {
            switch self {
            case .unauthenticated:
                return "Abra o app Verbalix para fazer login antes de usar esta extensão."
            case .noText:
                return "Nenhum texto foi encontrado. Selecione texto antes de usar o Verbalix."
            case .textTooLong:
                return "O texto excede 12.000 caracteres. Selecione uma parte menor."
            case .timeout:
                return "A IA não respondeu a tempo. Tente novamente."
            case .rateLimited:
                return "Limite de requisições atingido. Aguarde alguns instantes."
            case .general(let msg):
                return msg
            }
        }
    }

    private let scrollView = UIScrollView()
    private let contentStack = UIStackView()
    private let resultLabel = UILabel()
    private let activityIndicator = UIActivityIndicatorView(style: .large)

    private var currentState: State = .loading {
        didSet { applyState() }
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        setupUI()
        extractAndTransform()
    }

    private func setupUI() {
        view.backgroundColor = .systemGroupedBackground
        title = "Verbalix"

        navigationItem.leftBarButtonItem = UIBarButtonItem(
            barButtonSystemItem: .close,
            target: self,
            action: #selector(dismissExtension)
        )

        scrollView.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(scrollView)
        NSLayoutConstraint.activate([
            scrollView.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            scrollView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            scrollView.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor)
        ])

        contentStack.axis = .vertical
        contentStack.spacing = 16
        contentStack.layoutMargins = UIEdgeInsets(top: 20, left: 20, bottom: 20, right: 20)
        contentStack.isLayoutMarginsRelativeArrangement = true
        contentStack.translatesAutoresizingMaskIntoConstraints = false
        scrollView.addSubview(contentStack)
        NSLayoutConstraint.activate([
            contentStack.topAnchor.constraint(equalTo: scrollView.topAnchor),
            contentStack.leadingAnchor.constraint(equalTo: scrollView.leadingAnchor),
            contentStack.trailingAnchor.constraint(equalTo: scrollView.trailingAnchor),
            contentStack.bottomAnchor.constraint(equalTo: scrollView.bottomAnchor),
            contentStack.widthAnchor.constraint(equalTo: scrollView.widthAnchor)
        ])

        activityIndicator.translatesAutoresizingMaskIntoConstraints = false
        activityIndicator.startAnimating()

        resultLabel.numberOfLines = 0
        resultLabel.font = .preferredFont(forTextStyle: .body)

        contentStack.addArrangedSubview(activityIndicator)
    }

    private func applyState() {
        contentStack.arrangedSubviews.forEach { $0.removeFromSuperview() }

        switch currentState {
        case .loading:
            activityIndicator.startAnimating()
            contentStack.addArrangedSubview(activityIndicator)

        case .result(let text):
            activityIndicator.stopAnimating()

            let notice = makeNoticeLabel(
                "O resultado não é inserido automaticamente no iOS. Copie e cole manualmente."
            )
            contentStack.addArrangedSubview(notice)

            resultLabel.text = text
            contentStack.addArrangedSubview(resultLabel)

            contentStack.addArrangedSubview(makeButton(title: "Copiar", style: .filled) { [weak self] in
                UIPasteboard.general.string = text
                self?.dismissExtension()
            })

            contentStack.addArrangedSubview(makeButton(title: "Compartilhar", style: .plain) { [weak self] in
                let activity = UIActivityViewController(activityItems: [text], applicationActivities: nil)
                self?.present(activity, animated: true)
            })

        case .error(let error):
            activityIndicator.stopAnimating()

            let titleLabel = UILabel()
            titleLabel.text = error.title
            titleLabel.font = .preferredFont(forTextStyle: .title2)
            titleLabel.textAlignment = .center
            contentStack.addArrangedSubview(titleLabel)

            let messageLabel = UILabel()
            messageLabel.text = error.message
            messageLabel.font = .preferredFont(forTextStyle: .body)
            messageLabel.textColor = .secondaryLabel
            messageLabel.numberOfLines = 0
            messageLabel.textAlignment = .center
            contentStack.addArrangedSubview(messageLabel)

            if case .unauthenticated = error {
                contentStack.addArrangedSubview(makeButton(title: "Abrir Verbalix", style: .filled) { [weak self] in
                    self?.openMainApp()
                })
            }

            contentStack.addArrangedSubview(makeButton(title: "Fechar", style: .plain) { [weak self] in
                self?.dismissExtension()
            })
        }
    }

    private func extractAndTransform() {
        guard let inputItems = extensionContext?.inputItems as? [NSExtensionItem] else {
            currentState = .error(.noText)
            return
        }

        Task {
            let text = await extractPlainText(from: inputItems)
            guard let text, !text.isEmpty else {
                currentState = .error(.noText)
                return
            }

            guard text.unicodeScalars.count <= 12_000 else {
                currentState = .error(.textTooLong)
                return
            }

            await performTransform(text: text)
        }
    }

    private func extractPlainText(from items: [NSExtensionItem]) async -> String? {
        for item in items {
            guard let attachments = item.attachments else { continue }
            for provider in attachments {
                if provider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) {
                    let loaded = try? await provider.loadItem(
                        forTypeIdentifier: UTType.plainText.identifier
                    )
                    if let text = loaded as? String { return text }
                    if let data = loaded as? Data { return String(data: data, encoding: .utf8) }
                }
            }
        }
        return nil
    }

    private func performTransform(text: String) async {
        guard let config = BackendConfig(infoPlist: Bundle.main.infoDictionary ?? [:]) else {
            currentState = .error(.general("Configuração indisponível."))
            return
        }

        let sessionStore = SharedSessionStore(
            service: "com.verbalix.session",
            accessGroup: "com.verbalix.shared"
        )
        let refresher = SessionRefresher(
            config: config,
            store: sessionStore,
            appGroupID: "group.com.verbalix.shared"
        )

        let token: String
        do {
            token = try await refresher.validAccessToken()
        } catch {
            currentState = .error(.unauthenticated)
            return
        }

        let transport = URLSessionTransport(timeout: 15)
        let client = TransformClient(transport: transport, config: config)
        let request = TransformRequest(
            operation: .translate,
            text: text,
            preferences: nil
        )

        do {
            let response = try await client.transform(request, accessToken: token)
            currentState = .result(response.result)
        } catch VerbalixError.providerTimeout {
            currentState = .error(.timeout)
        } catch VerbalixError.rateLimited {
            currentState = .error(.rateLimited)
        } catch VerbalixError.unauthenticated {
            currentState = .error(.unauthenticated)
        } catch let error as VerbalixError {
            currentState = .error(.general(ErrorMessages.message(for: error)))
        } catch {
            currentState = .error(.general("Erro de conexão."))
        }
    }

    @objc private func dismissExtension() {
        extensionContext?.completeRequest(returningItems: [], completionHandler: nil)
    }

    private func openMainApp() {
        guard let url = URL(string: "verbalix-ios://") else {
            dismissExtension()
            return
        }
        extensionContext?.open(url, completionHandler: { [weak self] _ in
            self?.dismissExtension()
        })
    }

    private enum ButtonStyle { case filled, plain }

    private func makeButton(title: String, style: ButtonStyle, action: @escaping () -> Void) -> UIButton {
        let button = UIButton(type: .system)
        button.setTitle(title, for: .normal)
        button.titleLabel?.font = .preferredFont(forTextStyle: .headline)
        button.layer.cornerRadius = 12
        button.contentEdgeInsets = UIEdgeInsets(top: 14, left: 20, bottom: 14, right: 20)

        switch style {
        case .filled:
            button.backgroundColor = .systemBlue
            button.setTitleColor(.white, for: .normal)
        case .plain:
            button.backgroundColor = .secondarySystemGroupedBackground
            button.setTitleColor(.label, for: .normal)
        }

        button.addAction(UIAction { _ in action() }, for: .touchUpInside)
        return button
    }

    private func makeNoticeLabel(_ text: String) -> UILabel {
        let label = UILabel()
        label.text = text
        label.font = .preferredFont(forTextStyle: .footnote)
        label.textColor = .secondaryLabel
        label.numberOfLines = 0
        label.textAlignment = .center
        return label
    }
}
