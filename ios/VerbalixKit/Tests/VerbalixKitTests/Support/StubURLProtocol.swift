import Foundation

final class StubURLProtocol: URLProtocol, @unchecked Sendable {
    struct Stub {
        let status: Int
        let body: Data
    }

    static let host = "verbalix-session-refresher-test.local"

    private static let lock = NSLock()
    private nonisolated(unsafe) static var stub: Stub?
    private nonisolated(unsafe) static var capturedBodies: [Data] = []

    static func setStub(status: Int, body: Data) {
        lock.withLock { stub = Stub(status: status, body: body) }
    }

    static func reset() {
        lock.withLock {
            stub = nil
            capturedBodies = []
        }
    }

    static func lastCapturedBody() -> Data? {
        lock.withLock { capturedBodies.last }
    }

    static func requestCount() -> Int {
        lock.withLock { capturedBodies.count }
    }

    override class func canInit(with request: URLRequest) -> Bool {
        request.url?.host == host
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        let bodyData = bodyData(from: request)
        let currentStub = Self.lock.withLock { () -> Stub? in
            if let bodyData {
                Self.capturedBodies.append(bodyData)
            }
            return Self.stub
        }

        guard let currentStub, let url = request.url else {
            client?.urlProtocol(self, didFailWithError: URLError(.badServerResponse))
            return
        }

        let response = HTTPURLResponse(
            url: url,
            statusCode: currentStub.status,
            httpVersion: "HTTP/1.1",
            headerFields: ["Content-Type": "application/json"]
        )!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: currentStub.body)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}

    private func bodyData(from request: URLRequest) -> Data? {
        if let body = request.httpBody {
            return body
        }
        guard let stream = request.httpBodyStream else { return nil }
        stream.open()
        defer { stream.close() }
        var data = Data()
        let bufferSize = 4096
        var buffer = [UInt8](repeating: 0, count: bufferSize)
        while stream.hasBytesAvailable {
            let read = stream.read(&buffer, maxLength: bufferSize)
            if read <= 0 { break }
            data.append(buffer, count: read)
        }
        return data
    }
}
