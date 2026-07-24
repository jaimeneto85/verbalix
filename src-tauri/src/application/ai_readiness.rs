use crate::domain::VerbalixError;
use serde::Serialize;

mod embedded_backend_config {
    include!(concat!(env!("OUT_DIR"), "/verbalix_backend_config.rs"));
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicBackendConfig {
    pub supabase_url: String,
    pub anonymous_key: String,
    pub configured: bool,
}

impl PublicBackendConfig {
    pub fn resolve() -> Self {
        Self::from_pairs([
            process_pair("VITE_SUPABASE_URL", "VITE_SUPABASE_ANON_KEY"),
            process_pair("VERBALIX_SUPABASE_URL", "VERBALIX_SUPABASE_ANON_KEY"),
            complete_pair(
                embedded_backend_config::VITE_URL,
                embedded_backend_config::VITE_KEY,
            ),
            complete_pair(
                embedded_backend_config::LEGACY_URL,
                embedded_backend_config::LEGACY_KEY,
            ),
        ])
    }

    fn from_pairs(pairs: [Option<(String, String)>; 4]) -> Self {
        let (supabase_url, anonymous_key) = pairs.into_iter().flatten().next().unwrap_or_default();
        let supabase_url = supabase_url.trim_end_matches('/').to_owned();
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

fn process_pair(url_name: &str, key_name: &str) -> Option<(String, String)> {
    complete_pair(
        std::env::var(url_name).ok().as_deref(),
        std::env::var(key_name).ok().as_deref(),
    )
}

fn complete_pair(url: Option<&str>, key: Option<&str>) -> Option<(String, String)> {
    let url = url?.trim();
    let key = key?.trim();
    (!url.is_empty() && !key.is_empty()).then(|| (url.to_owned(), key.to_owned()))
}

fn valid_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://localhost")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_process_pair_has_highest_priority() {
        let config = PublicBackendConfig::from_pairs([
            complete_pair(Some("http://localhost:54321/"), Some("canonical-process")),
            complete_pair(
                Some("https://legacy-process.supabase.co"),
                Some("legacy-process"),
            ),
            complete_pair(
                Some("https://canonical-embedded.supabase.co"),
                Some("canonical-embedded"),
            ),
            complete_pair(
                Some("https://legacy-embedded.supabase.co"),
                Some("legacy-embedded"),
            ),
        ]);

        assert_eq!(config.supabase_url, "http://localhost:54321");
        assert_eq!(config.anonymous_key, "canonical-process");
        assert!(config.configured);
    }

    #[test]
    fn complete_legacy_process_pair_overrides_embedded_bundle_for_development() {
        let config = PublicBackendConfig::from_pairs([
            None,
            complete_pair(Some("http://localhost:54321"), Some("legacy-process")),
            complete_pair(
                Some("https://canonical-embedded.supabase.co"),
                Some("canonical-embedded"),
            ),
            None,
        ]);

        assert_eq!(config.supabase_url, "http://localhost:54321");
        assert_eq!(config.anonymous_key, "legacy-process");
        assert!(config.configured);
    }

    #[test]
    fn canonical_embedded_pair_makes_finder_launches_self_contained() {
        let config = PublicBackendConfig::from_pairs([
            None,
            None,
            complete_pair(Some("https://project.supabase.co/"), Some("public-anon")),
            complete_pair(
                Some("https://legacy.supabase.co"),
                Some("legacy-public-anon"),
            ),
        ]);

        assert_eq!(
            config.transform_endpoint(),
            "https://project.supabase.co/functions/v1/transform"
        );
        assert!(config.configured);
    }

    #[test]
    fn incomplete_canonical_pair_falls_back_without_mixing_sources() {
        let config = PublicBackendConfig::from_pairs([
            complete_pair(Some("https://canonical.supabase.co"), None),
            complete_pair(Some("http://localhost:54321"), Some("legacy-pair")),
            None,
            None,
        ]);

        assert_eq!(config.supabase_url, "http://localhost:54321");
        assert_eq!(config.anonymous_key, "legacy-pair");
    }

    #[test]
    fn incomplete_or_invalid_public_configuration_is_not_ready() {
        let missing_key = PublicBackendConfig::from_pairs([None, None, None, None]);
        let unsafe_url = PublicBackendConfig::from_pairs([
            complete_pair(Some("file:///tmp/backend"), Some("public-anon")),
            None,
            None,
            None,
        ]);

        assert!(!missing_key.configured);
        assert!(!unsafe_url.configured);
    }

    #[test]
    fn safe_smoke_reports_only_readiness_without_configuration_values() {
        let config = PublicBackendConfig::from_pairs([
            complete_pair(
                Some("https://configured.example.invalid"),
                Some("fixture-anon-key"),
            ),
            None,
            None,
            None,
        ]);
        let readiness = evaluate_ai_readiness(config.configured, false);

        assert_eq!(readiness.status, AiReadinessStatus::LoginRequired);
        assert!(!readiness.message.contains(&config.supabase_url));
        assert!(!readiness.message.contains(&config.anonymous_key));

        let missing = PublicBackendConfig::from_pairs([None, None, None, None]);
        assert_eq!(
            evaluate_ai_readiness(missing.configured, false).status,
            AiReadinessStatus::ProviderNotConfigured
        );
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
