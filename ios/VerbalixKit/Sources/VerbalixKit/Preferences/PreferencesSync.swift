import Foundation

private struct RemotePreferencesRow: Codable {
    let formality: Int
    let length: LengthPreference
    let tone: TonePreference
    let historyEnabled: Bool
    let updatedAt: Date?

    enum CodingKeys: String, CodingKey {
        case formality
        case length
        case tone
        case historyEnabled = "history_enabled"
        case updatedAt = "updated_at"
    }
}

private struct UpsertBody: Codable {
    let formality: Int
    let length: LengthPreference
    let tone: TonePreference
    let historyEnabled: Bool

    enum CodingKeys: String, CodingKey {
        case formality
        case length
        case tone
        case historyEnabled = "history_enabled"
    }
}

public struct PreferencesSync: Sendable {
    let transport: HTTPTransport
    let config: BackendConfig

    public init(transport: HTTPTransport = URLSessionTransport(), config: BackendConfig) {
        self.transport = transport
        self.config = config
    }

    public func fetch(accessToken: String) async throws -> SyncedPreferences? {
        var components = URLComponents(url: config.preferencesEndpoint, resolvingAgainstBaseURL: false)!
        components.queryItems = [
            URLQueryItem(name: "select", value: "*"),
            URLQueryItem(name: "limit", value: "1")
        ]

        var request = URLRequest(url: components.url!)
        request.httpMethod = "GET"
        request.setValue(config.anonKey, forHTTPHeaderField: "apikey")
        request.setValue("Bearer \(accessToken)", forHTTPHeaderField: "Authorization")

        let (data, response): (Data, HTTPURLResponse)
        do {
            (data, response) = try await transport.send(request)
        } catch let error as VerbalixError {
            throw error
        } catch {
            throw VerbalixError.transport(error)
        }

        if response.statusCode == 401 {
            throw VerbalixError.unauthenticated
        }

        guard (200..<300).contains(response.statusCode) else {
            throw VerbalixError.providerRejected
        }

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601

        guard let rows = try? decoder.decode([RemotePreferencesRow].self, from: data) else {
            throw VerbalixError.invalidResponse
        }

        guard let first = rows.first else { return nil }

        return SyncedPreferences(
            formality: first.formality,
            length: first.length,
            tone: first.tone,
            historyEnabled: first.historyEnabled,
            updatedAt: first.updatedAt
        )
    }

    public func upsert(_ prefs: SyncedPreferences, accessToken: String) async throws {
        let body = UpsertBody(
            formality: prefs.formality,
            length: prefs.length,
            tone: prefs.tone,
            historyEnabled: prefs.historyEnabled
        )

        let encoder = JSONEncoder()
        let data = try encoder.encode(body)

        var request = URLRequest(url: config.preferencesEndpoint)
        request.httpMethod = "POST"
        request.httpBody = data
        request.setValue(config.anonKey, forHTTPHeaderField: "apikey")
        request.setValue("Bearer \(accessToken)", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("resolution=merge-duplicates,return=minimal", forHTTPHeaderField: "Prefer")

        let (_, response): (Data, HTTPURLResponse)
        do {
            (_, response) = try await transport.send(request)
        } catch let error as VerbalixError {
            throw error
        } catch {
            throw VerbalixError.transport(error)
        }

        if response.statusCode == 401 {
            throw VerbalixError.unauthenticated
        }

        guard (200..<300).contains(response.statusCode) else {
            throw VerbalixError.providerRejected
        }
    }
}

public func mergePreferences(local: SyncedPreferences, remote: SyncedPreferences?) -> SyncedPreferences {
    guard let remote = remote else {
        return local
    }

    guard let remoteDate = remote.updatedAt else {
        return local
    }

    guard let localDate = local.updatedAt else {
        return SyncedPreferences(
            formality: remote.formality,
            length: remote.length,
            tone: remote.tone,
            historyEnabled: remote.historyEnabled,
            updatedAt: remote.updatedAt
        )
    }

    if remoteDate > localDate {
        return SyncedPreferences(
            formality: remote.formality,
            length: remote.length,
            tone: remote.tone,
            historyEnabled: remote.historyEnabled,
            updatedAt: remote.updatedAt
        )
    }

    return local
}
