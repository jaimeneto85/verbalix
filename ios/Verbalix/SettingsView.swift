import SwiftUI
import VerbalixKit

struct SettingsView: View {
    @Environment(AppSession.self) private var session

    @State private var formality: Double = Double(PreferencesStore.defaults.formality)
    @State private var tone: TonePreference = PreferencesStore.defaults.tone
    @State private var length: LengthPreference = PreferencesStore.defaults.length
    @State private var historyEnabled: Bool = PreferencesStore.defaults.historyEnabled
    @State private var saveError: String? = nil
    @State private var debounceTask: Task<Void, Never>? = nil

    private var store: PreferencesStore {
        PreferencesStore(directory: preferencesDirectory)
    }

    private var preferencesDirectory: URL {
        FileManager.default
            .containerURL(forSecurityApplicationGroupIdentifier: "group.com.verbalix.shared")
            ?? FileManager.default.temporaryDirectory
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Text("Formalidade")
                            Spacer()
                            Text("\(Int(formality))")
                                .foregroundStyle(.secondary)
                                .monospacedDigit()
                        }
                        Slider(value: $formality, in: 1...5, step: 1)
                            .onChange(of: formality) { _, _ in scheduleUpsert() }
                    }
                } header: {
                    Text("Estilo de escrita")
                }

                Section {
                    Picker("Tom", selection: $tone) {
                        Text("Neutro").tag(TonePreference.neutral)
                        Text("Amigável").tag(TonePreference.friendly)
                        Text("Assertivo").tag(TonePreference.assertive)
                        Text("Técnico").tag(TonePreference.technical)
                    }
                    .pickerStyle(.segmented)
                    .onChange(of: tone) { _, _ in scheduleUpsert() }

                    Picker("Comprimento", selection: $length) {
                        Text("Conciso").tag(LengthPreference.concise)
                        Text("Balanceado").tag(LengthPreference.balanced)
                        Text("Detalhado").tag(LengthPreference.detailed)
                    }
                    .pickerStyle(.segmented)
                    .onChange(of: length) { _, _ in scheduleUpsert() }
                } header: {
                    Text("Tom e comprimento")
                }

                Section {
                    Toggle("Salvar histórico", isOn: $historyEnabled)
                        .onChange(of: historyEnabled) { _, _ in scheduleUpsert() }
                } header: {
                    Text("Histórico")
                } footer: {
                    Text("Quando ativado, as transformações são armazenadas por 30 dias.")
                }

                if let error = saveError {
                    Section {
                        Text(error)
                            .font(.footnote)
                            .foregroundStyle(.red)
                    }
                }

                Section {
                    Button("Sair", role: .destructive) {
                        Task { await session.signOut() }
                    }
                }
            }
            .navigationTitle("Ajustes")
            .task { await loadPreferences() }
        }
    }

    private func loadPreferences() async {
        guard let loaded = try? store.load() else { return }
        formality = Double(loaded.formality)
        tone = loaded.tone
        length = loaded.length
        historyEnabled = loaded.historyEnabled
    }

    private func scheduleUpsert() {
        debounceTask?.cancel()
        debounceTask = Task {
            try? await Task.sleep(for: .milliseconds(600))
            guard !Task.isCancelled else { return }
            await persistAndSync()
        }
    }

    private func persistAndSync() async {
        var prefs = SyncedPreferences(
            formality: Int(formality),
            length: length,
            tone: tone,
            historyEnabled: historyEnabled
        )
        prefs.updatedAt = Date()

        do {
            try store.save(prefs)
        } catch {
            saveError = "Falha ao salvar preferências localmente."
            return
        }
        saveError = nil

        guard let token = session.accessToken else { return }
        guard let config = BackendConfig(infoPlist: Bundle.main.infoDictionary ?? [:]) else { return }
        let sync = PreferencesSync(config: config)
        try? await sync.upsert(prefs, accessToken: token)
    }
}
