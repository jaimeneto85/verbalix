import SwiftUI
import VerbalixKit

private enum HistoryLoadState {
    case loading
    case loaded([HistoryItem])
    case empty
    case error(String)
}

struct HistoryView: View {
    @Environment(AppSession.self) private var session

    @State private var state: HistoryLoadState = .loading
    @State private var showDeleteAllConfirmation = false

    var body: some View {
        NavigationStack {
            Group {
                switch state {
                case .loading:
                    ProgressView()
                        .frame(maxWidth: .infinity, maxHeight: .infinity)

                case .empty:
                    ContentUnavailableView(
                        "Sem histórico",
                        systemImage: "clock",
                        description: Text("As transformações realizadas aparecerão aqui.")
                    )

                case .error(let message):
                    ContentUnavailableView(
                        "Erro ao carregar",
                        systemImage: "exclamationmark.triangle",
                        description: Text(message)
                    )

                case .loaded(let items):
                    List {
                        ForEach(items) { item in
                            HistoryItemRow(item: item, onCopy: { copy(item) })
                        }
                        .onDelete { indexSet in
                            deleteItems(at: indexSet, in: items)
                        }
                    }
                }
            }
            .navigationTitle("Histórico")
            .toolbar {
                if case .loaded(let items) = state, !items.isEmpty {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button("Apagar tudo", role: .destructive) {
                            showDeleteAllConfirmation = true
                        }
                    }
                }
            }
            .confirmationDialog(
                "Apagar todo o histórico?",
                isPresented: $showDeleteAllConfirmation,
                titleVisibility: .visible
            ) {
                Button("Apagar tudo", role: .destructive) {
                    Task { await deleteAll() }
                }
            }
            .task { await loadHistory() }
        }
    }

    private func loadHistory() async {
        guard let token = session.accessToken else {
            state = .error("Não autenticado.")
            return
        }
        guard let config = BackendConfig(infoPlist: Bundle.main.infoDictionary ?? [:]) else {
            state = .error("Configuração indisponível.")
            return
        }
        let client = HistoryClient(config: config)
        do {
            let items = try await client.list(accessToken: token)
            state = items.isEmpty ? .empty : .loaded(items)
        } catch let error as VerbalixError {
            state = .error(ErrorMessages.message(for: error))
        } catch {
            state = .error("Erro de conexão.")
        }
    }

    private func copy(_ item: HistoryItem) {
        UIPasteboard.general.string = item.resultText
    }

    private func deleteItems(at indexSet: IndexSet, in items: [HistoryItem]) {
        let toDelete = indexSet.map { items[$0] }
        Task { await deleteItems(toDelete) }
    }

    private func deleteItems(_ items: [HistoryItem]) async {
        guard let token = session.accessToken,
              let config = BackendConfig(infoPlist: Bundle.main.infoDictionary ?? [:]) else { return }
        let client = HistoryClient(config: config)
        for item in items {
            try? await client.delete(id: item.id, accessToken: token)
        }
        await loadHistory()
    }

    private func deleteAll() async {
        guard let token = session.accessToken,
              let config = BackendConfig(infoPlist: Bundle.main.infoDictionary ?? [:]) else { return }
        let client = HistoryClient(config: config)
        try? await client.deleteAll(accessToken: token)
        await loadHistory()
    }
}

private struct HistoryItemRow: View {
    let item: HistoryItem
    let onCopy: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(item.operation.capitalized)
                    .font(.caption.weight(.semibold))
                    .padding(.horizontal, 8)
                    .padding(.vertical, 2)
                    .background(Color.accentColor.opacity(0.15), in: .capsule)
                Spacer()
                Text(formattedDate)
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
            }
            Text(item.resultText)
                .font(.subheadline)
                .lineLimit(3)
        }
        .swipeActions(edge: .leading) {
            Button(action: onCopy) {
                Label("Copiar", systemImage: "doc.on.doc")
            }
            .tint(.blue)
        }
    }

    private var formattedDate: String {
        let isoString = item.createdAt
        let formatter = ISO8601DateFormatter()
        guard let date = formatter.date(from: isoString) else { return isoString }
        let display = DateFormatter()
        display.dateStyle = .short
        display.timeStyle = .short
        return display.string(from: date)
    }
}

extension HistoryItem: @retroactive Identifiable {}
