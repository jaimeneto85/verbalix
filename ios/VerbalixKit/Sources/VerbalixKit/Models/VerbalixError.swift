import Foundation

public enum VerbalixError: Error {
    case invalidResponse
    case textTooLong
    case unauthenticated
    case providerNotConfigured
    case loginRequired
    case providerTimeout
    case providerRejected
    case rateLimited
    case localFailure
    case transport(Error)
    case authLinkExpired
    case authLinkAlreadyUsed
}

extension VerbalixError {
    public init?(serverCode: String) {
        switch serverCode {
        case "UNAUTHENTICATED": self = .unauthenticated
        case "TEXT_TOO_LONG": self = .textTooLong
        case "RATE_LIMITED": self = .rateLimited
        case "PROVIDER_TIMEOUT": self = .providerTimeout
        case "INVALID_RESPONSE": self = .invalidResponse
        case "INTERNAL_ERROR": self = .providerRejected
        default: return nil
        }
    }
}
