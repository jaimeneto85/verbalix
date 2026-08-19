use serde::Serialize;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct LiveSessionId(pub Uuid);

impl LiveSessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Ord, PartialOrd)]
pub struct SegmentId(pub u64);

impl SegmentId {
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

const ALLOWED_LANGUAGES: &[&str] = &["en", "pt", "es", "fr", "de", "it", "ja", "ko", "zh"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguageTag(String);

impl LanguageTag {
    pub fn parse(s: &str) -> Option<Self> {
        ALLOWED_LANGUAGES.contains(&s).then(|| Self(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LiveState {
    Idle,
    Preparing,
    OnAir,
    Recovering,
    Stopping,
    Failed,
}

#[derive(Clone, Debug)]
pub struct LiveSession {
    pub id: LiveSessionId,
    pub next_segment: SegmentId,
    pub target_language: LanguageTag,
}

impl LiveSession {
    pub fn new(target_language: LanguageTag) -> Self {
        Self {
            id: LiveSessionId::new(),
            next_segment: SegmentId(0),
            target_language,
        }
    }

    pub fn accepts(&self, session_id: LiveSessionId, _segment_id: SegmentId) -> bool {
        self.id == session_id
    }

    pub fn advance(&mut self) -> SegmentId {
        let id = self.next_segment;
        self.next_segment = id.next();
        id
    }
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct StageDurations {
    pub stt: u32,
    pub translate: u32,
    pub tts: u32,
}

#[derive(Debug)]
pub struct SegmentResult {
    pub audio_base64: String,
    pub detected_language: String,
    pub stage_ms: StageDurations,
}

#[derive(Debug)]
pub struct InterpretOutcome {
    pub session_id: LiveSessionId,
    pub segment_id: SegmentId,
    pub result: Result<SegmentResult, crate::domain::VerbalixError>,
}

const CONTEXT_CAP_ITEMS: usize = 2;
const CONTEXT_SOURCE_CAP_CHARS: usize = 300;

pub struct TranslationContext {
    items: Vec<String>,
}

impl TranslationContext {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn push(&mut self, source: &str) {
        let capped: String = source.chars().take(CONTEXT_SOURCE_CAP_CHARS).collect();
        if self.items.len() >= CONTEXT_CAP_ITEMS {
            self.items.remove(0);
        }
        self.items.push(capped);
    }

    pub fn snapshot(&self) -> Vec<String> {
        self.items.clone()
    }

    pub fn reset(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
#[path = "live_interpretation_tests.rs"]
mod tests;
