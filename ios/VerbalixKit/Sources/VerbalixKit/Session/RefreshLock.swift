import Darwin
import Foundation

public struct RefreshLock: Sendable {
    private let lockPath: String
    private let timeoutSeconds: Double

    public init(appGroupID: String, timeoutSeconds: Double = 12) {
        let base = FileManager.default
            .containerURL(forSecurityApplicationGroupIdentifier: appGroupID)
            ?? FileManager.default.temporaryDirectory
        lockPath = base.appendingPathComponent(".refresh.lock").path
        self.timeoutSeconds = timeoutSeconds
    }

    init(lockPath: String, timeoutSeconds: Double = 12) {
        self.lockPath = lockPath
        self.timeoutSeconds = timeoutSeconds
    }

    public func withLock<T: Sendable>(_ body: () async throws -> T) async throws -> T {
        let fd = try openLockFile()
        defer {
            releaseLock(fd: fd)
            Darwin.close(fd)
        }
        try await acquireLock(fd: fd)
        return try await body()
    }

    private func openLockFile() throws -> Int32 {
        let fd = Darwin.open(lockPath, O_RDWR | O_CREAT | O_CLOEXEC, 0o644)
        guard fd >= 0 else { throw VerbalixError.localFailure }
        return fd
    }

    private func tryAcquireExclusive(fd: Int32) -> Bool {
        var fl = Darwin.flock(
            l_start: 0, l_len: 0, l_pid: 0,
            l_type: Int16(F_WRLCK), l_whence: Int16(SEEK_SET)
        )
        return fcntl(fd, F_SETLK, &fl) == 0
    }

    private func releaseLock(fd: Int32) {
        var fl = Darwin.flock(
            l_start: 0, l_len: 0, l_pid: 0,
            l_type: Int16(F_UNLCK), l_whence: Int16(SEEK_SET)
        )
        _ = fcntl(fd, F_SETLK, &fl)
    }

    private func acquireLock(fd: Int32) async throws {
        let deadline = Date().timeIntervalSince1970 + timeoutSeconds
        while Date().timeIntervalSince1970 < deadline {
            if tryAcquireExclusive(fd: fd) {
                touchLock(fd)
                return
            }
            if isLockHolderDead(fd: fd), tryAcquireExclusive(fd: fd) {
                touchLock(fd)
                return
            }
            try await Task.sleep(nanoseconds: 50_000_000)
        }
        throw VerbalixError.providerTimeout
    }

    private func isLockHolderDead(fd: Int32) -> Bool {
        var fl = Darwin.flock(
            l_start: 0, l_len: 0, l_pid: 0,
            l_type: Int16(F_WRLCK), l_whence: Int16(SEEK_SET)
        )
        guard fcntl(fd, F_GETLK, &fl) == 0 else { return false }
        guard fl.l_type != Int16(F_UNLCK) else { return false }
        return Darwin.kill(fl.l_pid, 0) != 0
    }

    private func touchLock(_ fd: Int32) {
        var zero: UInt8 = 0
        _ = Darwin.write(fd, &zero, 1)
        _ = Darwin.ftruncate(fd, 0)
    }
}
