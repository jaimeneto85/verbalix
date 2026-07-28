import Darwin
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

    @Test func contentionTimesOutWhileALiveProcessHoldsTheLock() async throws {
        let path = tempLockPath()
        let holder = try ExternalLockHolder(path: path, holdSeconds: 3)
        defer { holder.terminate() }

        let lock = RefreshLock(lockPath: path, timeoutSeconds: 0.6)
        await #expect(throws: VerbalixError.providerTimeout) {
            try await lock.withLock {}
        }
    }

    @Test func lockBecomesAvailableAfterHolderProcessIsKilled() async throws {
        let path = tempLockPath()
        let holder = try ExternalLockHolder(path: path, holdSeconds: 30)
        holder.kill()

        let lock = RefreshLock(lockPath: path, timeoutSeconds: 3)
        var executed = false
        try await lock.withLock { executed = true }
        #expect(executed)
    }

    @Test func secondAcquireProceedsOnlyAfterFirstHolderProcessReleases() async throws {
        let path = tempLockPath()
        let holder = try ExternalLockHolder(path: path, holdSeconds: 1)
        defer { holder.terminate() }

        let started = Date()
        let lock = RefreshLock(lockPath: path, timeoutSeconds: 5)
        var executed = false
        try await lock.withLock { executed = true }
        let elapsed = Date().timeIntervalSince(started)

        #expect(executed)
        #expect(elapsed >= 0.8, "acquisition must wait for the external holder to release, not race past it")
    }
}

private final class ExternalLockHolder {
    private let process: Process

    init(path: String, holdSeconds: Double) throws {
        let marker = FileManager.default.temporaryDirectory
            .appendingPathComponent("verbalix-test-marker-\(UUID().uuidString)").path

        let script = """
        import fcntl, sys, time
        fd = open(sys.argv[1], 'a+')
        fcntl.lockf(fd, fcntl.LOCK_EX)
        open(sys.argv[2], 'w').close()
        time.sleep(float(sys.argv[3]))
        """

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["python3", "-c", script, path, marker, String(holdSeconds)]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try process.run()
        self.process = process

        let deadline = Date().addingTimeInterval(5)
        while !FileManager.default.fileExists(atPath: marker), Date() < deadline {
            usleep(10_000)
        }
        guard FileManager.default.fileExists(atPath: marker) else {
            process.terminate()
            throw VerbalixError.localFailure
        }
        try? FileManager.default.removeItem(atPath: marker)
    }

    func terminate() {
        if process.isRunning {
            process.terminate()
        }
    }

    func kill() {
        Darwin.kill(process.processIdentifier, SIGKILL)
        process.waitUntilExit()
    }
}
