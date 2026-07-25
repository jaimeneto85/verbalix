import Foundation
import Testing
@testable import VerbalixKit

struct RefreshLockTests {

    private func tempLockPath() -> String {
        FileManager.default.temporaryDirectory
            .appendingPathComponent("verbalix-test-\(UUID().uuidString).lock")
            .path
    }

    @Test func acquiresAndReleasesLock() async throws {
        let lock = RefreshLock(lockPath: tempLockPath(), timeoutSeconds: 5)
        var executed = false
        try await lock.withLock {
            executed = true
        }
        #expect(executed)
    }

    @Test func bodyValueIsReturned() async throws {
        let lock = RefreshLock(lockPath: tempLockPath(), timeoutSeconds: 5)
        let result = try await lock.withLock { 42 }
        #expect(result == 42)
    }

    @Test func throwsFromBodyPropagates() async throws {
        let lock = RefreshLock(lockPath: tempLockPath(), timeoutSeconds: 5)
        do {
            try await lock.withLock { throw VerbalixError.invalidResponse }
            Issue.record("Expected throw not raised")
        } catch VerbalixError.invalidResponse {
        }
    }

    @Test func createsLockFileOnDisk() async throws {
        let path = tempLockPath()
        let lock = RefreshLock(lockPath: path, timeoutSeconds: 5)
        try await lock.withLock {}
        #expect(FileManager.default.fileExists(atPath: path))
    }

    @Test func serialisesSequentialCallsOnSameInstance() async throws {
        let lock = RefreshLock(lockPath: tempLockPath(), timeoutSeconds: 5)
        var counter = 0
        for _ in 0..<5 {
            try await lock.withLock { counter += 1 }
        }
        #expect(counter == 5)
    }

    @Test func orphanedLockIsRecovered() async throws {
        let path = tempLockPath()
        _ = FileManager.default.createFile(atPath: path, contents: nil)

        let lock = RefreshLock(lockPath: path, timeoutSeconds: 5)
        var executed = false
        try await lock.withLock { executed = true }
        #expect(executed)
    }
}
