use crate::domain::{LengthPreference, TonePreference, VerbalixError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TransformOperation {
    Translate,
    Improve,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformPreferences {
    pub formality: u8,
    pub length: LengthPreference,
    pub tone: TonePreference,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformRequest {
    pub request_id: Uuid,
    pub operation: TransformOperation,
    pub text: String,
    pub preferences: Option<TransformPreferences>,
}

impl TransformRequest {
    pub fn validate(&self) -> Result<(), VerbalixError> {
        if self.text.trim().is_empty() {
            return Err(VerbalixError::InvalidResponse);
        }
        if self.text.chars().count() > 12_000 {
            return Err(VerbalixError::TextTooLong);
        }
        if self
            .preferences
            .as_ref()
            .is_some_and(|preferences| !(1..=5).contains(&preferences.formality))
        {
            return Err(VerbalixError::InvalidResponse);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformResult {
    pub request_id: Uuid,
    pub source_language: String,
    pub target_language: Option<String>,
    pub result: String,
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    async fn transform(
        &self,
        request: &TransformRequest,
        access_token: &str,
    ) -> Result<TransformResult, VerbalixError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_unicode_scalars_for_limit() {
        let request = TransformRequest {
            request_id: Uuid::new_v4(),
            operation: TransformOperation::Translate,
            text: "👩🏽‍💻".repeat(3_001),
            preferences: None,
        };
        assert!(matches!(request.validate(), Err(VerbalixError::TextTooLong)));
    }

    #[test]
    fn accepts_technical_text_at_boundary() {
        let request = TransformRequest {
            request_id: Uuid::new_v4(),
            operation: TransformOperation::Improve,
            text: "a".repeat(12_000),
            preferences: Some(TransformPreferences {
                formality: 3,
                length: LengthPreference::Balanced,
                tone: TonePreference::Technical,
            }),
        };
        assert!(request.validate().is_ok());
    }
}
