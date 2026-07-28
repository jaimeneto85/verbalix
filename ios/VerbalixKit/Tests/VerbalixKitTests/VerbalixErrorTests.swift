import XCTest
@testable import VerbalixKit

final class VerbalixErrorTests: XCTestCase {
    private let serverCodeExpectations: [(code: String, error: VerbalixError, message: String)] = [
        ("UNAUTHENTICATED", .unauthenticated, "Autenticação remota necessária."),
        ("TEXT_TOO_LONG", .textTooLong, "O texto selecionado excede o limite de 12.000 caracteres."),
        ("RATE_LIMITED", .rateLimited, "Limite de requisições atingido. Tente novamente em instantes."),
        ("PROVIDER_TIMEOUT", .providerTimeout, "O provedor de IA não respondeu a tempo."),
        ("INVALID_RESPONSE", .invalidResponse, "A resposta recebida é inválida."),
        ("INTERNAL_ERROR", .providerRejected, "O provedor de IA rejeitou a solicitação."),
    ]

    func testEachServerCodeMapsToTheExpectedVerbalixError() {
        for expectation in serverCodeExpectations {
            let error = VerbalixError(serverCode: expectation.code)
            XCTAssertEqual(error, expectation.error, "server code \(expectation.code)")
        }
    }

    func testEachServerCodeMapsToItsPortugueseMessage() {
        for expectation in serverCodeExpectations {
            let error = VerbalixError(serverCode: expectation.code)!
            XCTAssertEqual(ErrorMessages.message(for: error), expectation.message, "server code \(expectation.code)")
        }
    }

    func testUnknownServerCodeReturnsNil() {
        XCTAssertNil(VerbalixError(serverCode: "SOMETHING_ELSE"))
        XCTAssertNil(VerbalixError(serverCode: ""))
        XCTAssertNil(VerbalixError(serverCode: "unauthenticated"))
    }

    func testClientOnlyErrorsHavePortugueseMessages() {
        let expectations: [(VerbalixError, String)] = [
            (.providerNotConfigured, "O provedor de IA não está configurado."),
            (.loginRequired, "É necessário fazer login para continuar."),
            (.localFailure, "Falha ao acessar dados locais."),
            (.transport(StubHTTPTransportError()), "Erro de conexão com o servidor."),
        ]

        for (error, message) in expectations {
            XCTAssertEqual(ErrorMessages.message(for: error), message)
        }
    }
}
