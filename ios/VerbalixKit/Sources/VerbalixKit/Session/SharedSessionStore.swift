import Foundation
import Security

public protocol SessionPersisting: Sendable {
    func load() throws -> StoredSession?
    func save(_ session: StoredSession) throws
    func clear() throws
}

public struct StoredSession: Codable, Sendable {
    public let accessToken: String
    public let refreshToken: String

    public init(accessToken: String, refreshToken: String) {
        self.accessToken = accessToken
        self.refreshToken = refreshToken
    }
}

public final class InMemorySessionStore: @unchecked Sendable, SessionPersisting {
    private var stored: StoredSession?

    public init() {}

    public func load() throws -> StoredSession? { stored }

    public func save(_ session: StoredSession) throws { stored = session }

    public func clear() throws { stored = nil }
}

public struct KeychainSessionStore: SessionPersisting {
    let service: String
    let accessGroup: String

    private let account = "session"

    public init(service: String, accessGroup: String) {
        self.service = service
        self.accessGroup = accessGroup
    }

    public func load() throws -> StoredSession? {
        var query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account,
            kSecReturnData: true,
            kSecMatchLimit: kSecMatchLimitOne
        ]
        if !accessGroup.isEmpty {
            query[kSecAttrAccessGroup] = accessGroup
        }

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)

        if status == errSecItemNotFound { return nil }

        guard status == errSecSuccess, let data = result as? Data else {
            throw VerbalixError.localFailure
        }

        guard let session = try? JSONDecoder().decode(StoredSession.self, from: data) else {
            throw VerbalixError.localFailure
        }

        return session
    }

    public func save(_ session: StoredSession) throws {
        guard let data = try? JSONEncoder().encode(session) else {
            throw VerbalixError.localFailure
        }

        var query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account
        ]
        if !accessGroup.isEmpty {
            query[kSecAttrAccessGroup] = accessGroup
        }

        var attributes: [CFString: Any] = [
            kSecValueData: data,
            kSecAttrAccessible: kSecAttrAccessibleAfterFirstUnlock
        ]
        if !accessGroup.isEmpty {
            attributes[kSecAttrAccessGroup] = accessGroup
        }

        let updateStatus = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)

        if updateStatus == errSecItemNotFound {
            var addQuery: [CFString: Any] = [
                kSecClass: kSecClassGenericPassword,
                kSecAttrService: service,
                kSecAttrAccount: account,
                kSecValueData: data,
                kSecAttrAccessible: kSecAttrAccessibleAfterFirstUnlock
            ]
            if !accessGroup.isEmpty {
                addQuery[kSecAttrAccessGroup] = accessGroup
            }

            let addStatus = SecItemAdd(addQuery as CFDictionary, nil)
            guard addStatus == errSecSuccess else {
                throw VerbalixError.localFailure
            }
        } else if updateStatus != errSecSuccess {
            throw VerbalixError.localFailure
        }
    }

    public func clear() throws {
        var query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account
        ]
        if !accessGroup.isEmpty {
            query[kSecAttrAccessGroup] = accessGroup
        }

        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw VerbalixError.localFailure
        }
    }
}

public final class SharedSessionStore: Sendable {
    let store: any SessionPersisting

    public init(store: any SessionPersisting) {
        self.store = store
    }

    public convenience init(service: String, accessGroup: String = "com.verbalix.shared") {
        self.init(store: KeychainSessionStore(service: service, accessGroup: accessGroup))
    }

    public func load() throws -> StoredSession? {
        try store.load()
    }

    public func save(_ session: StoredSession) throws {
        try store.save(session)
    }

    public func clear() throws {
        try store.clear()
    }
}
