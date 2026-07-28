import Auth
import Foundation
import Security

public final class VerbalixAuthStorage: AuthLocalStorage, @unchecked Sendable {
    private let sessionStore: SharedSessionStore
    private let service: String
    private let accessGroup: String

    public init(sessionStore: SharedSessionStore, service: String, accessGroup: String) {
        self.sessionStore = sessionStore
        self.service = service
        self.accessGroup = accessGroup
    }

    public func store(key: String, value: Data) throws {
        try rawStore(key: key, data: value)
        syncToSessionStore(from: value)
    }

    public func retrieve(key: String) throws -> Data? {
        do {
            return try rawRetrieve(key: key)
        } catch {
            return nil
        }
    }

    public func remove(key: String) throws {
        try rawRemove(key: key)
        try? sessionStore.clear()
    }

    private func rawStore(key: String, data: Data) throws {
        var query = baseQuery(for: key)
        query[kSecReturnData] = false

        let exists = SecItemCopyMatching(query as CFDictionary, nil) == errSecSuccess
        if exists {
            let update: [CFString: Any] = [kSecValueData: data]
            let status = SecItemUpdate(query as CFDictionary, update as CFDictionary)
            guard status == errSecSuccess else { throw VerbalixError.localFailure }
        } else {
            var add = query
            add[kSecValueData] = data
            add[kSecAttrAccessible] = kSecAttrAccessibleAfterFirstUnlock
            let status = SecItemAdd(add as CFDictionary, nil)
            guard status == errSecSuccess else { throw VerbalixError.localFailure }
        }
    }

    private func rawRetrieve(key: String) throws -> Data {
        var query = baseQuery(for: key)
        query[kSecReturnData] = true
        query[kSecMatchLimit] = kSecMatchLimitOne

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)

        guard status == errSecSuccess, let data = result as? Data else {
            throw VerbalixError.localFailure
        }
        return data
    }

    private func rawRemove(key: String) throws {
        let query = baseQuery(for: key)
        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw VerbalixError.localFailure
        }
    }

    private func baseQuery(for key: String) -> [CFString: Any] {
        var q: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: key
        ]
        if !accessGroup.isEmpty {
            q[kSecAttrAccessGroup] = accessGroup
        }
        return q
    }

    private func syncToSessionStore(from data: Data) {
        struct TokenPayload: Decodable {
            let access_token: String
            let refresh_token: String
        }
        guard let payload = try? JSONDecoder().decode(TokenPayload.self, from: data) else { return }
        let session = StoredSession(accessToken: payload.access_token, refreshToken: payload.refresh_token)
        try? sessionStore.save(session)
    }
}
