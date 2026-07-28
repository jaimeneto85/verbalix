import UIKit
import VerbalixKit

final class KeyboardViewController: UIInputViewController {
    private let toolbar = UIView()
    private let translateButton = UIButton(type: .system)
    private let improveButton = UIButton(type: .system)
    private let progressIndicator = UIActivityIndicatorView(style: .medium)
    private let hintLabel = UILabel()
    private let fullAccessBanner = UIView()

    private var isTransforming = false {
        didSet { updateButtonState() }
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        setupToolbar()
        setupFullAccessBanner()
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        isTransforming = false
        updateAccessState()
    }

    private func setupToolbar() {
        toolbar.backgroundColor = .systemGroupedBackground
        toolbar.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(toolbar)

        NSLayoutConstraint.activate([
            toolbar.topAnchor.constraint(equalTo: view.topAnchor),
            toolbar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            toolbar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            toolbar.heightAnchor.constraint(equalToConstant: 44)
        ])

        configureActionButton(translateButton, title: "Traduzir")
        translateButton.addAction(UIAction { [weak self] _ in
            self?.performTransform(operation: .translate)
        }, for: .touchUpInside)

        configureActionButton(improveButton, title: "Aprimorar")
        improveButton.addAction(UIAction { [weak self] _ in
            self?.performTransform(operation: .improve)
        }, for: .touchUpInside)

        progressIndicator.hidesWhenStopped = true
        progressIndicator.translatesAutoresizingMaskIntoConstraints = false

        hintLabel.text = "Selecione texto para traduzir ou aprimorar"
        hintLabel.font = .preferredFont(forTextStyle: .caption1)
        hintLabel.textColor = .secondaryLabel
        hintLabel.textAlignment = .center
        hintLabel.translatesAutoresizingMaskIntoConstraints = false

        let buttonStack = UIStackView(arrangedSubviews: [translateButton, improveButton])
        buttonStack.axis = .horizontal
        buttonStack.spacing = 12
        buttonStack.translatesAutoresizingMaskIntoConstraints = false

        toolbar.addSubview(buttonStack)
        toolbar.addSubview(progressIndicator)
        toolbar.addSubview(hintLabel)

        NSLayoutConstraint.activate([
            buttonStack.centerYAnchor.constraint(equalTo: toolbar.centerYAnchor),
            buttonStack.leadingAnchor.constraint(equalTo: toolbar.leadingAnchor, constant: 16),
            progressIndicator.centerYAnchor.constraint(equalTo: toolbar.centerYAnchor),
            progressIndicator.trailingAnchor.constraint(equalTo: toolbar.trailingAnchor, constant: -16),
            hintLabel.centerYAnchor.constraint(equalTo: toolbar.centerYAnchor),
            hintLabel.leadingAnchor.constraint(equalTo: buttonStack.trailingAnchor, constant: 8),
            hintLabel.trailingAnchor.constraint(equalTo: progressIndicator.leadingAnchor, constant: -8)
        ])
    }

    private func setupFullAccessBanner() {
        fullAccessBanner.backgroundColor = .systemOrange.withAlphaComponent(0.1)
        fullAccessBanner.layer.cornerRadius = 8
        fullAccessBanner.translatesAutoresizingMaskIntoConstraints = false
        fullAccessBanner.isHidden = true
        view.addSubview(fullAccessBanner)

        let bannerLabel = UILabel()
        bannerLabel.numberOfLines = 0
        bannerLabel.font = .preferredFont(forTextStyle: .caption1)
        bannerLabel.text = "O Acesso Total está desativado. O teclado Verbalix precisa de Acesso Total para consultar a IA."
        bannerLabel.translatesAutoresizingMaskIntoConstraints = false
        fullAccessBanner.addSubview(bannerLabel)

        let settingsButton = UIButton(type: .system)
        settingsButton.setTitle("Ajustes", for: .normal)
        settingsButton.titleLabel?.font = .preferredFont(forTextStyle: .caption1).bold()
        settingsButton.translatesAutoresizingMaskIntoConstraints = false
        settingsButton.addAction(UIAction { [weak self] _ in
            self?.openSettings()
        }, for: .touchUpInside)
        fullAccessBanner.addSubview(settingsButton)

        NSLayoutConstraint.activate([
            fullAccessBanner.topAnchor.constraint(equalTo: toolbar.bottomAnchor, constant: 4),
            fullAccessBanner.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 8),
            fullAccessBanner.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -8),
            bannerLabel.topAnchor.constraint(equalTo: fullAccessBanner.topAnchor, constant: 8),
            bannerLabel.leadingAnchor.constraint(equalTo: fullAccessBanner.leadingAnchor, constant: 12),
            bannerLabel.trailingAnchor.constraint(equalTo: settingsButton.leadingAnchor, constant: -8),
            bannerLabel.bottomAnchor.constraint(equalTo: fullAccessBanner.bottomAnchor, constant: -8),
            settingsButton.centerYAnchor.constraint(equalTo: fullAccessBanner.centerYAnchor),
            settingsButton.trailingAnchor.constraint(equalTo: fullAccessBanner.trailingAnchor, constant: -12)
        ])

        view.heightAnchor.constraint(equalToConstant: 120).isActive = true
    }

    private func updateAccessState() {
        let hasAccess = hasFullAccess
        fullAccessBanner.isHidden = hasAccess
        translateButton.isEnabled = hasAccess
        improveButton.isEnabled = hasAccess
    }

    private func updateButtonState() {
        translateButton.isEnabled = !isTransforming && hasFullAccess
        improveButton.isEnabled = !isTransforming && hasFullAccess

        if isTransforming {
            progressIndicator.startAnimating()
            hintLabel.isHidden = true
        } else {
            progressIndicator.stopAnimating()
            hintLabel.isHidden = false
        }
    }

    private func performTransform(operation: TransformOperation) {
        guard hasFullAccess else { return }

        guard let selected = textDocumentProxy.selectedText, !selected.isEmpty else {
            hintLabel.text = "Selecione texto para traduzir ou aprimorar"
            return
        }

        guard selected.unicodeScalars.count <= 12_000 else {
            hintLabel.text = "Texto muito longo (máx 12.000 caracteres)"
            return
        }

        guard let config = BackendConfig(infoPlist: Bundle.main.infoDictionary ?? [:]) else {
            hintLabel.text = "Configuração indisponível"
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

        isTransforming = true

        Task { [weak self] in
            guard let self else { return }

            defer {
                Task { @MainActor in self.isTransforming = false }
            }

            let token: String
            do {
                token = try await refresher.validAccessToken()
            } catch {
                await MainActor.run {
                    self.hintLabel.text = "Faça login no app Verbalix primeiro"
                }
                return
            }

            let transport = URLSessionTransport(timeout: 15)
            let client = TransformClient(transport: transport, config: config)
            let prefs = TransformPreferences(formality: 3, length: .balanced, tone: .technical)
            let request = TransformRequest(
                operation: operation,
                text: selected,
                preferences: operation == .improve ? prefs : nil
            )

            do {
                let response = try await client.transform(request, accessToken: token)
                await MainActor.run {
                    let selectedLength = (selected as NSString).length
                    for _ in 0..<selectedLength {
                        self.textDocumentProxy.deleteBackward()
                    }
                    self.textDocumentProxy.insertText(response.result)
                    self.hintLabel.text = "Selecione texto para traduzir ou aprimorar"
                }
            } catch VerbalixError.providerTimeout {
                await MainActor.run { self.hintLabel.text = "Tempo esgotado. Tente novamente." }
            } catch VerbalixError.rateLimited {
                await MainActor.run { self.hintLabel.text = "Limite atingido. Aguarde." }
            } catch VerbalixError.textTooLong {
                await MainActor.run { self.hintLabel.text = "Texto muito longo." }
            } catch {
                await MainActor.run { self.hintLabel.text = "Erro. Tente novamente." }
            }
        }
    }

    private func openSettings() {
        guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
        extensionContext?.open(url, completionHandler: nil)
    }

    private func configureActionButton(_ button: UIButton, title: String) {
        button.setTitle(title, for: .normal)
        button.titleLabel?.font = .preferredFont(forTextStyle: .subheadline).bold()
        button.backgroundColor = .systemBlue
        button.setTitleColor(.white, for: .normal)
        button.layer.cornerRadius = 8
        button.contentEdgeInsets = UIEdgeInsets(top: 6, left: 14, bottom: 6, right: 14)
        button.translatesAutoresizingMaskIntoConstraints = false
    }
}

private extension UIFont {
    func bold() -> UIFont {
        guard let descriptor = fontDescriptor.withSymbolicTraits(.traitBold) else { return self }
        return UIFont(descriptor: descriptor, size: pointSize)
    }
}
