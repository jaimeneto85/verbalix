import Foundation
import Testing
@testable import VerbalixKit

@Suite(.serialized)
struct SessionRefresherTests {
    private let config = BackendConfig(
        supabaseURL: URL(string: "https://\(StubURLProtocol.host)")!,
        anonKey: "anon-key"
    )!

    private func makeRefresher(store: InMemorySessionStore) -> SessionRefresher {
        URLProtocol.registerClass(StubURLProtocol.self)
        let lockPath = FileManager.default.temporaryDirectory
            .appendingPathComponent("refresh-\(UUID().uuidString).lock").path
        return SessionRefresher(
            config: config,
            store: SharedSessionStore(store: store),
            lock: RefreshLock(lockPath: lockPath)
        )
    }

    private func teardown() {
        URLProtocol.unregisterClass(StubURLProtocol.self)
        StubURLProtocol.reset()
    }

    @Test func validAccessTokenReturnsStoredTokenWithoutNetworkCall() async throws {
        defer { teardown() }
        let store = InMemorySessionStore()
        try store.save(StoredSession(accessToken: "current-token", refreshToken: "current-refresh"))
        let refresher = makeRefresher(store: store)

        let token = try await refresher.validAccessToken()

        #expect(token == "current-token")
        #expect(StubURLProtocol.requestCount() == 0)
    }

    @Test func validAccessTokenThrowsUnauthenticatedWhenNoSessionIsStored() async throws {
        defer { teardown() }
        let refresher = makeRefresher(store: InMemorySessionStore())

        await #expect(throws: VerbalixError.unauthenticated) {
            _ = try await refresher.validAccessToken()
        }
    }

    @Test func refreshAndGetTokenThrowsUnauthenticatedWhenNoSessionIsStored() async throws {
        defer { teardown() }
        let refresher = makeRefresher(store: InMemorySessionStore())

        await #expect(throws: VerbalixError.unauthenticated) {
            _ = try await refresher.refreshAndGetToken()
        }
        #expect(StubURLProtocol.requestCount() == 0)
    }

    @Test func refreshAndGetTokenSucceedsAndPersistsTheRefreshedSession() async throws {
        defer { teardown() }
        let store = InMemorySessionStore()
        try store.save(StoredSession(accessToken: "old-token", refreshToken: "old-refresh"))
        let refresher = makeRefresher(store: store)
        let body = try JSONEncoder().encode(["access_token": "new-token", "refresh_token": "new-refresh"])
        StubURLProtocol.setStub(status: 200, body: body)

        let token = try await refresher.refreshAndGetToken()

        #expect(token == "new-token")
        let persisted = try store.load()
        #expect(persisted?.accessToken == "new-token")
        #expect(persisted?.refreshToken == "new-refresh")

        let sentBody = try #require(StubURLProtocol.lastCapturedBody())
        let sentJSON = try #require(String(data: sentBody, encoding: .utf8))
        #expect(sentJSON.contains("old-refresh"), "the refresh request must send the previously stored refresh token")
    }

    @Test func refreshAndGetTokenMapsUnauthorizedResponseToUnauthenticated() async throws {
        defer { teardown() }
        let store = InMemorySessionStore()
        try store.save(StoredSession(accessToken: "old-token", refreshToken: "old-refresh"))
        let refresher = makeRefresher(store: store)
        StubURLProtocol.setStub(status: 401, body: Data())

        await #expect(throws: VerbalixError.unauthenticated) {
            _ = try await refresher.refreshAndGetToken()
        }

        let persisted = try store.load()
        #expect(persisted?.accessToken == "old-token", "a rejected refresh must never overwrite the still-valid stored session")
    }

    @Test func refreshAndGetTokenMapsBadRequestResponseToUnauthenticated() async throws {
        defer { teardown() }
        let store = InMemorySessionStore()
        try store.save(StoredSession(accessToken: "old-token", refreshToken: "old-refresh"))
        let refresher = makeRefresher(store: store)
        StubURLProtocol.setStub(status: 400, body: Data())

        await #expect(throws: VerbalixError.unauthenticated) {
            _ = try await refresher.refreshAndGetToken()
        }
    }

    @Test func refreshAndGetTokenMapsServerErrorToProviderRejected() async throws {
        defer { teardown() }
        let store = InMemorySessionStore()
        try store.save(StoredSession(accessToken: "old-token", refreshToken: "old-refresh"))
        let refresher = makeRefresher(store: store)
        StubURLProtocol.setStub(status: 503, body: Data())

        await #expect(throws: VerbalixError.providerRejected) {
            _ = try await refresher.refreshAndGetToken()
        }
    }

    @Test func refreshAndGetTokenMapsMalformedBodyToInvalidResponse() async throws {
        defer { teardown() }
        let store = InMemorySessionStore()
        try store.save(StoredSession(accessToken: "old-token", refreshToken: "old-refresh"))
        let refresher = makeRefresher(store: store)
        StubURLProtocol.setStub(status: 200, body: "not json".data(using: .utf8)!)

        await #expect(throws: VerbalixError.invalidResponse) {
            _ = try await refresher.refreshAndGetToken()
        }
    }

    @Test func concurrentRefreshesAreSerializedAndBothObserveTheFinalSession() async throws {
        defer { teardown() }
        let store = InMemorySessionStore()
        try store.save(StoredSession(accessToken: "old-token", refreshToken: "old-refresh"))
        let refresher = makeRefresher(store: store)
        let body = try JSONEncoder().encode(["access_token": "new-token", "refresh_token": "new-refresh"])
        StubURLProtocol.setStub(status: 200, body: body)

        async let first = refresher.refreshAndGetToken()
        async let second = refresher.refreshAndGetToken()
        let results = try await [first, second]

        #expect(results.allSatisfy { $0 == "new-token" })
        #expect(StubURLProtocol.requestCount() == 2, "the lock must serialize both refreshes rather than dropping one")
    }
}
