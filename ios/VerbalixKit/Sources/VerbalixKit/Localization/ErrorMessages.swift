import Foundation

public enum ErrorMessages {
    public static func message(for error: VerbalixError) -> String {
        switch error {
        case .textTooLong:
            return "O texto selecionado excede o limite de 12.000 caracteres."
        case .unauthenticated:
            return "Autenticação remota necessária."
        case .providerNotConfigured:
            return "O provedor de IA não está configurado."
        case .loginRequired:
            return "É necessário fazer login para continuar."
        case .providerTimeout:
            return "O provedor de IA não respondeu a tempo."
        case .providerRejected:
            return "O provedor de IA rejeitou a solicitação."
        case .rateLimited:
            return "Limite de requisições atingido. Tente novamente em instantes."
        case .invalidResponse:
            return "A resposta recebida é inválida."
        case .localFailure:
            return "Falha ao acessar dados locais."
        case .transport:
            return "Erro de conexão com o servidor."
        }
    }
}
