import XCTest
@testable import VerbalixKit

final class PreferencesStoreTests: XCTestCase {
    private var tempDirectory: URL!

    override func setUpWithError() throws {
        tempDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent("verbalixkit-tests-\(UUID().uuidString)")
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: tempDirectory)
    }

    func testLoadReturnsDefaultsWhenFileIsAbsent() throws {
        let store = PreferencesStore(directory: tempDirectory)

        let loaded = try store.load()

        XCTAssertEqual(loaded.formality, PreferencesStore.defaults.formality)
        XCTAssertEqual(loaded.length, PreferencesStore.defaults.length)
        XCTAssertEqual(loaded.tone, PreferencesStore.defaults.tone)
        XCTAssertEqual(loaded.historyEnabled, PreferencesStore.defaults.historyEnabled)
        XCTAssertNil(loaded.updatedAt)
    }

    func testSaveThenLoadRoundTripsAllFields() throws {
        let store = PreferencesStore(directory: tempDirectory)
        let prefs = SyncedPreferences(
            formality: 5,
            length: .detailed,
            tone: .assertive,
            historyEnabled: true,
            updatedAt: Date(timeIntervalSince1970: 1_753_401_600)
        )

        try store.save(prefs)
        let loaded = try store.load()

        XCTAssertEqual(loaded.formality, prefs.formality)
        XCTAssertEqual(loaded.length, prefs.length)
        XCTAssertEqual(loaded.tone, prefs.tone)
        XCTAssertEqual(loaded.historyEnabled, prefs.historyEnabled)
        let loadedUpdatedAt = try XCTUnwrap(loaded.updatedAt?.timeIntervalSince1970)
        let expectedUpdatedAt = try XCTUnwrap(prefs.updatedAt?.timeIntervalSince1970)
        XCTAssertEqual(loadedUpdatedAt, expectedUpdatedAt, accuracy: 1)
    }

    func testSaveOverwritesPreviousContentAtomically() throws {
        let store = PreferencesStore(directory: tempDirectory)
        try store.save(SyncedPreferences(formality: 1, length: .concise, tone: .neutral, historyEnabled: false))
        try store.save(SyncedPreferences(formality: 5, length: .detailed, tone: .friendly, historyEnabled: true))

        let loaded = try store.load()

        XCTAssertEqual(loaded.formality, 5)
        XCTAssertEqual(loaded.length, .detailed)
        XCTAssertEqual(loaded.tone, .friendly)
        XCTAssertTrue(loaded.historyEnabled)
    }

    func testLoadFailsClosedOnCorruptedFileInsteadOfResettingToDefaults() throws {
        let store = PreferencesStore(directory: tempDirectory)
        try FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true)
        let filePath = tempDirectory.appendingPathComponent("preferences.json")
        let corrupted = "{ not valid json".data(using: .utf8)!
        try corrupted.write(to: filePath)

        XCTAssertThrowsError(try store.load()) { error in
            XCTAssertEqual(error as? VerbalixError, .localFailure)
        }

        let stillCorrupted = try Data(contentsOf: filePath)
        XCTAssertEqual(stillCorrupted, corrupted, "a corrupted file must never be silently reset to defaults")
    }

    func testLoadFailsClosedWhenFileContainsAnUnrelatedJSONShape() throws {
        let store = PreferencesStore(directory: tempDirectory)
        try FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true)
        let filePath = tempDirectory.appendingPathComponent("preferences.json")
        try "{\"unexpected\":true}".data(using: .utf8)!.write(to: filePath)

        XCTAssertThrowsError(try store.load()) { error in
            XCTAssertEqual(error as? VerbalixError, .localFailure)
        }
    }

    func testRecordLocalChangeSetsUpdatedAtBeforePersisting() throws {
        let store = PreferencesStore(directory: tempDirectory)
        let beforeSecs = floor(Date().timeIntervalSince1970)

        try store.recordLocalChange()

        let loaded = try store.load()
        let updatedAt = try XCTUnwrap(loaded.updatedAt)
        XCTAssertGreaterThanOrEqual(floor(updatedAt.timeIntervalSince1970), beforeSecs)
    }

    func testRecordLocalChangeBumpsUpdatedAtOnSubsequentCall() throws {
        let store = PreferencesStore(directory: tempDirectory)
        var prefs = SyncedPreferences(formality: 3, length: .balanced, tone: .technical, historyEnabled: false)
        prefs.updatedAt = Date(timeIntervalSince1970: 1_000_000)
        try store.save(prefs)

        try store.recordLocalChange()

        let loaded = try store.load()
        let updatedAt = try XCTUnwrap(loaded.updatedAt)
        XCTAssertGreaterThan(updatedAt.timeIntervalSince1970, 1_000_000)
    }
}

final class PreferencesSyncSerializationTests: XCTestCase {
    private let config = BackendConfig(supabaseURL: URL(string: "https://project.supabase.co")!, anonKey: "anon-key")!

    func testUpsertSerializesSnakeCaseAIFieldsOnly() async throws {
        let transport = StubHTTPTransport.success(status: 200, body: Data())
        let sync = PreferencesSync(transport: transport, config: config)
        let prefs = SyncedPreferences(formality: 4, length: .detailed, tone: .friendly, historyEnabled: true)

        try await sync.upsert(prefs, accessToken: "token")

        let body = try XCTUnwrap(transport.lastRequestBody)
        let json = try XCTUnwrap(String(data: body, encoding: .utf8))

        XCTAssertTrue(json.contains("\"formality\":4"))
        XCTAssertTrue(json.contains("\"length\":\"detailed\""))
        XCTAssertTrue(json.contains("\"tone\":\"friendly\""))
        XCTAssertTrue(json.contains("\"history_enabled\":true"))
        XCTAssertFalse(json.contains("updated_at"), "server owns updated_at via a trigger; client must never send it")

        let sentRequest = try XCTUnwrap(transport.capturedRequests.first)
        XCTAssertEqual(sentRequest.value(forHTTPHeaderField: "Prefer"), "resolution=merge-duplicates,return=minimal")
    }

    func testFetchDecodesSnakeCaseRowIntoSyncedPreferences() async throws {
        let body = """
        [{"formality":2,"length":"concise","tone":"neutral","history_enabled":false,"updated_at":"2026-07-25T00:00:00Z"}]
        """.data(using: .utf8)!
        let transport = StubHTTPTransport.success(status: 200, body: body)
        let sync = PreferencesSync(transport: transport, config: config)

        let fetched = try await sync.fetch(accessToken: "token")

        let prefs = try XCTUnwrap(fetched)
        XCTAssertEqual(prefs.formality, 2)
        XCTAssertEqual(prefs.length, .concise)
        XCTAssertEqual(prefs.tone, .neutral)
        XCTAssertFalse(prefs.historyEnabled)
        XCTAssertNotNil(prefs.updatedAt)
    }

    func testFetchReturnsNilWhenNoRowExistsYet() async throws {
        let transport = StubHTTPTransport.success(status: 200, body: "[]".data(using: .utf8)!)
        let sync = PreferencesSync(transport: transport, config: config)

        let fetched = try await sync.fetch(accessToken: "token")

        XCTAssertNil(fetched)
    }
}

final class MergePreferencesTests: XCTestCase {
    private func prefs(formality: Int, updatedAt: Date?) -> SyncedPreferences {
        SyncedPreferences(formality: formality, length: .balanced, tone: .technical, historyEnabled: false, updatedAt: updatedAt)
    }

    func testRemoteWinsWhenStrictlyNewer() {
        let local = prefs(formality: 1, updatedAt: Date(timeIntervalSince1970: 100))
        let remote = prefs(formality: 2, updatedAt: Date(timeIntervalSince1970: 200))

        let (merged, needsPush) = mergePreferences(local: local, remote: remote)

        XCTAssertEqual(merged.formality, 2)
        XCTAssertFalse(needsPush)
    }

    func testLocalWinsOnExactTie() {
        let sameInstant = Date(timeIntervalSince1970: 100)
        let local = prefs(formality: 1, updatedAt: sameInstant)
        let remote = prefs(formality: 2, updatedAt: sameInstant)

        let (merged, needsPush) = mergePreferences(local: local, remote: remote)

        XCTAssertEqual(merged.formality, 1)
        XCTAssertTrue(needsPush, "tie means local won and should push to keep server in sync")
    }

    func testLocalWinsWhenRemoteUpdatedAtIsMissing() {
        let local = prefs(formality: 1, updatedAt: Date(timeIntervalSince1970: 100))
        let remote = prefs(formality: 2, updatedAt: nil)

        let (merged, needsPush) = mergePreferences(local: local, remote: remote)

        XCTAssertEqual(merged.formality, 1)
        XCTAssertTrue(needsPush)
    }

    func testLocalWinsWhenRemoteUpdatedAtIsMissingAndLocalHasNoTimestamp() {
        let local = prefs(formality: 1, updatedAt: nil)
        let remote = prefs(formality: 2, updatedAt: nil)

        let (merged, needsPush) = mergePreferences(local: local, remote: remote)

        XCTAssertEqual(merged.formality, 1)
        XCTAssertFalse(needsPush)
    }

    func testRemoteWinsWhenLocalHasNoTimestampYet() {
        let local = prefs(formality: 1, updatedAt: nil)
        let remote = prefs(formality: 2, updatedAt: Date(timeIntervalSince1970: 200))

        let (merged, _) = mergePreferences(local: local, remote: remote)

        XCTAssertEqual(merged.formality, 2)
    }

    func testLocalIsReturnedWhenNoRemoteRowExists() {
        let local = prefs(formality: 1, updatedAt: Date(timeIntervalSince1970: 100))

        let (merged, needsPush) = mergePreferences(local: local, remote: nil)

        XCTAssertEqual(merged.formality, 1)
        XCTAssertFalse(needsPush)
    }

    func testLocalWinsWhenRemoteIsOlder() {
        let local = prefs(formality: 1, updatedAt: Date(timeIntervalSince1970: 200))
        let remote = prefs(formality: 2, updatedAt: Date(timeIntervalSince1970: 100))

        let (merged, needsPush) = mergePreferences(local: local, remote: remote)

        XCTAssertEqual(merged.formality, 1)
        XCTAssertTrue(needsPush)
    }
}
