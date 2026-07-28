import Foundation
@testable import VerbalixKit

extension VerbalixError: Equatable {
    public static func == (lhs: VerbalixError, rhs: VerbalixError) -> Bool {
        switch (lhs, rhs) {
        case (.invalidResponse, .invalidResponse),
             (.textTooLong, .textTooLong),
             (.unauthenticated, .unauthenticated),
             (.providerNotConfigured, .providerNotConfigured),
             (.loginRequired, .loginRequired),
             (.providerTimeout, .providerTimeout),
             (.providerRejected, .providerRejected),
             (.rateLimited, .rateLimited),
             (.localFailure, .localFailure),
             (.transport, .transport),
             (.authLinkExpired, .authLinkExpired),
             (.authLinkAlreadyUsed, .authLinkAlreadyUsed):
            return true
        default:
            return false
        }
    }
}
