use crate::domain::VerbalixError;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicBackendConfig {
    pub supabase_url: String,
    pub anonymous_key: String,
    pub configured: bool,
}

impl PublicBackendConfig {
    pub fn resolve() -> Self {
        Self::from_sources(
            std::env::var("VERBALIX_SUPABASE_URL").ok(),
            option_env!("VERBALIX_SUPABASE_URL"),
            std::env::var("VERBALIX_SUPABASE_ANON_KEY").ok(),
            option_env!("VERBALIX_SUPABASE_ANON_KEY"),
        )
    }

    fn from_sources(
        runtime_url: Option<String>,
        embedded_url: Option<&str>,
        runtime_key: Option<String>,
        embedded_key: Option<&str>,
    ) -> Self {
        let supabase_url = preferred(runtime_url, embedded_url)
            .trim_end_matches('/')
            .to_owned();
        let anonymous_key = preferred(runtime_key, embedded_key);
        let configured = valid_url(&supabase_url) && !anonymous_key.trim().is_empty();
        Self {
            supabase_url,
            anonymous_key,
            configured,
        }
    }

    pub fn transform_endpoint(&self) -> String {
        format!("{}/functions/v1/transform", self.supabase_url)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AiReadinessStatus {
    Ready,
    LoginRequired,
    ProviderNotConfigured,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshFailureRoute {
    LoginRequired,
    ProviderUnavailable,
}

impl AiReadinessStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::LoginRequired => "login_required",
            Self::ProviderNotConfigured => "provider_not_configured",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiReadiness {
    pub status: AiReadinessStatus,
    pub message: &'static str,
}

impl AiReadiness {
    pub fn ready() -> Self {
        Self {
            status: AiReadinessStatus::Ready,
            message: "A IA está pronta.",
        }
    }

    pub fn login_required() -> Self {
        Self {
            status: AiReadinessStatus::LoginRequired,
            message: "Entre no Verbalix para usar tradução e aprimoramento.",
        }
    }

    pub fn provider_not_configured() -> Self {
        Self {
            status: AiReadinessStatus::ProviderNotConfigured,
            message: "Este build não contém a configuração pública do backend.",
        }
    }
}

pub fn evaluate_ai_readiness(configured: bool, has_session: bool) -> AiReadiness {
    if !configured {
        AiReadiness::provider_not_configured()
    } else if !has_session {
        AiReadiness::login_required()
    } else {
        AiReadiness::ready()
    }
}

pub fn classify_refresh_failure(error: &VerbalixError) -> RefreshFailureRoute {
    match error {
        VerbalixError::Unauthenticated => RefreshFailureRoute::LoginRequired,
        _ => RefreshFailureRoute::ProviderUnavailable,
    }
}

fn preferred(runtime: Option<String>, embedded: Option<&str>) -> String {
    runtime
        .filter(|value| !value.trim().is_empty())
        .or_else(|| embedded.map(str::to_owned))
        .unwrap_or_default()
}

fn valid_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://localhost")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_values_override_embedded_build_values() {
        let config = PublicBackendConfig::from_sources(
            Some("http://localhost:54321/".to_owned()),
            Some("https://embedded.supabase.co"),
            Some("runtime-anon".to_owned()),
            Some("embedded-anon"),
        );

        assert_eq!(config.supabase_url, "http://localhost:54321");
        assert_eq!(config.anonymous_key, "runtime-anon");
        assert!(config.configured);
    }

    #[test]
    fn embedded_values_make_finder_launches_self_contained() {
        let config = PublicBackendConfig::from_sources(
            None,
            Some("https://project.supabase.co/"),
            None,
            Some("public-anon"),
        );

        assert_eq!(
            config.transform_endpoint(),
            "https://project.supabase.co/functions/v1/transform"
        );
        assert!(config.configured);
    }

    #[test]
    fn incomplete_or_invalid_public_configuration_is_not_ready() {
        let missing_key = PublicBackendConfig::from_sources(
            None,
            Some("https://project.supabase.co"),
            None,
            None,
        );
        let unsafe_url = PublicBackendConfig::from_sources(
            Some("file:///tmp/backend".to_owned()),
            None,
            Some("public-anon".to_owned()),
            None,
        );

        assert!(!missing_key.configured);
        assert!(!unsafe_url.configured);
    }

    #[test]
    fn readiness_blocks_configuration_before_requesting_login() {
        assert_eq!(
            evaluate_ai_readiness(false, false).status,
            AiReadinessStatus::ProviderNotConfigured
        );
        assert_eq!(
            evaluate_ai_readiness(true, false).status,
            AiReadinessStatus::LoginRequired
        );
        assert_eq!(
            evaluate_ai_readiness(true, true).status,
            AiReadinessStatus::Ready
        );
    }

    #[test]
    fn existing_session_refresh_errors_preserve_auth_for_provider_failures() {
        for error in [
            VerbalixError::ProviderRejected,
            VerbalixError::InvalidResponse,
        ] {
            assert_eq!(
                classify_refresh_failure(&error),
                RefreshFailureRoute::ProviderUnavailable
            );
        }
        assert_eq!(
            classify_refresh_failure(&VerbalixError::Unauthenticated),
            RefreshFailureRoute::LoginRequired
        );
    }
}
