use super::*;

#[test]
fn new_session_starts_at_segment_zero() {
    let lang = LanguageTag::parse("en").unwrap();
    let session = LiveSession::new(lang);
    assert_eq!(session.next_segment, SegmentId(0));
}

#[test]
fn advance_increments_segment_id() {
    let lang = LanguageTag::parse("pt").unwrap();
    let mut session = LiveSession::new(lang);
    let first = session.advance();
    let second = session.advance();
    assert_eq!(first, SegmentId(0));
    assert_eq!(second, SegmentId(1));
    assert_eq!(session.next_segment, SegmentId(2));
}

#[test]
fn accepts_returns_false_for_wrong_session() {
    let lang = LanguageTag::parse("en").unwrap();
    let session = LiveSession::new(lang);
    let other_id = LiveSessionId::new();
    assert!(!session.accepts(other_id, SegmentId(0)));
    assert!(session.accepts(session.id, SegmentId(0)));
}

#[test]
fn language_tag_rejects_unsupported_locale() {
    assert!(LanguageTag::parse("xx").is_none());
    assert!(LanguageTag::parse("").is_none());
    assert!(LanguageTag::parse("zz").is_none());
    assert!(LanguageTag::parse("EN").is_none());
}

#[test]
fn language_tag_accepts_all_allowlist_entries() {
    let allowed = ["en", "pt", "es", "fr", "de", "it", "ja", "ko", "zh"];
    for lang in &allowed {
        assert!(
            LanguageTag::parse(lang).is_some(),
            "Expected {lang} to be accepted"
        );
    }
}
