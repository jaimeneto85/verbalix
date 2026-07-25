import Foundation

public struct SyncedPreferences: Codable, Sendable {
    public var formality: Int
    public var length: LengthPreference
    public var tone: TonePreference
    public var historyEnabled: Bool
    public var updatedAt: Date?

    public init(formality: Int, length: LengthPreference, tone: TonePreference, historyEnabled: Bool, updatedAt: Date? = nil) {
        self.formality = formality
        self.length = length
        self.tone = tone
        self.historyEnabled = historyEnabled
        self.updatedAt = updatedAt
    }
}

public struct PreferencesStore: Sendable {
    let directory: URL

    public init(directory: URL) {
        self.directory = directory
    }

    private var filePath: URL {
        directory.appendingPathComponent("preferences.json")
    }

    public static var defaults: SyncedPreferences {
        SyncedPreferences(
            formality: 3,
            length: .balanced,
            tone: .technical,
            historyEnabled: false,
            updatedAt: nil
        )
    }

    public func load() throws -> SyncedPreferences {
        guard FileManager.default.fileExists(atPath: filePath.path) else {
            return Self.defaults
        }

        let data: Data
        do {
            data = try Data(contentsOf: filePath)
        } catch {
            throw VerbalixError.localFailure
        }

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        guard let prefs = try? decoder.decode(SyncedPreferences.self, from: data) else {
            throw VerbalixError.localFailure
        }

        return prefs
    }

    public func save(_ prefs: SyncedPreferences) throws {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601

        let data: Data
        do {
            data = try encoder.encode(prefs)
        } catch {
            throw VerbalixError.localFailure
        }

        let tmpPath = filePath.appendingPathExtension("tmp")

        do {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            try data.write(to: tmpPath, options: .atomic)
            _ = try FileManager.default.replaceItemAt(filePath, withItemAt: tmpPath)
        } catch {
            try? FileManager.default.removeItem(at: tmpPath)
            throw VerbalixError.localFailure
        }
    }

    public func recordLocalChange() throws {
        var prefs = try load()
        prefs.updatedAt = Date()
        try save(prefs)
    }
}
