import Foundation

public enum TransformOperation: String, Codable, Sendable {
    case translate
    case improve
}

public enum LengthPreference: String, Codable, Sendable {
    case concise
    case balanced
    case detailed
}

public enum TonePreference: String, Codable, Sendable {
    case neutral
    case friendly
    case assertive
    case technical
}

public struct TransformPreferences: Codable, Sendable {
    public let formality: Int
    public let length: LengthPreference
    public let tone: TonePreference

    public init(formality: Int, length: LengthPreference, tone: TonePreference) {
        self.formality = formality
        self.length = length
        self.tone = tone
    }
}

public struct TransformRequest: Codable, Sendable {
    public let requestId: UUID
    public let operation: TransformOperation
    public let text: String
    public let preferences: TransformPreferences?

    public init(requestId: UUID = UUID(), operation: TransformOperation, text: String, preferences: TransformPreferences? = nil) {
        self.requestId = requestId
        self.operation = operation
        self.text = text
        self.preferences = preferences
    }

    private enum CodingKeys: String, CodingKey {
        case requestId
        case operation
        case text
        case preferences
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(requestId.uuidString.lowercased(), forKey: .requestId)
        try container.encode(operation, forKey: .operation)
        try container.encode(text, forKey: .text)
        try container.encodeIfPresent(preferences, forKey: .preferences)
    }

    public func validated() throws -> TransformRequest {
        guard !text.isEmpty else {
            throw VerbalixError.invalidResponse
        }

        if text.unicodeScalars.count > 12_000 {
            throw VerbalixError.textTooLong
        }

        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        if let bodyData = try? encoder.encode(self), bodyData.count > 65_536 {
            throw VerbalixError.textTooLong
        }

        if operation == .improve {
            guard preferences != nil else {
                throw VerbalixError.invalidResponse
            }
        }

        if let prefs = preferences {
            guard (1...5).contains(prefs.formality) else {
                throw VerbalixError.invalidResponse
            }
        }

        return self
    }
}

public struct TransformResponse: Codable, Sendable {
    public let requestId: UUID
    public let sourceLanguage: String
    public let targetLanguage: String?
    public let result: String

    public init(requestId: UUID, sourceLanguage: String, targetLanguage: String? = nil, result: String) {
        self.requestId = requestId
        self.sourceLanguage = sourceLanguage
        self.targetLanguage = targetLanguage
        self.result = result
    }

    public func validated(for request: TransformRequest) throws -> TransformResponse {
        guard requestId == request.requestId else {
            throw VerbalixError.invalidResponse
        }

        guard !sourceLanguage.isEmpty else {
            throw VerbalixError.invalidResponse
        }

        guard !result.isEmpty else {
            throw VerbalixError.invalidResponse
        }

        if request.operation == .translate {
            guard let lang = targetLanguage, !lang.isEmpty else {
                throw VerbalixError.invalidResponse
            }
        }

        if request.operation == .improve {
            guard targetLanguage == nil else {
                throw VerbalixError.invalidResponse
            }
        }

        if result.unicodeScalars.count > 24_000 {
            throw VerbalixError.invalidResponse
        }

        return self
    }
}
