use super::*;

#[test]
fn translation_context_starts_empty() {
    let ctx = TranslationContext::new();
    assert!(ctx.snapshot().is_empty());
}

#[test]
fn translation_context_push_preserves_order() {
    let mut ctx = TranslationContext::new();
    ctx.push("first");
    ctx.push("second");
    let snap = ctx.snapshot();
    assert_eq!(snap[0], "first");
    assert_eq!(snap[1], "second");
}

#[test]
fn translation_context_cap_two_evicts_oldest() {
    let mut ctx = TranslationContext::new();
    ctx.push("a");
    ctx.push("b");
    ctx.push("c");
    let snap = ctx.snapshot();
    assert_eq!(snap.len(), 2);
    assert_eq!(snap[0], "b");
    assert_eq!(snap[1], "c");
}

#[test]
fn translation_context_reset_clears_all() {
    let mut ctx = TranslationContext::new();
    ctx.push("x");
    ctx.push("y");
    ctx.reset();
    assert!(ctx.snapshot().is_empty());
}

#[test]
fn translation_context_source_capped_at_300_scalars() {
    let long: String = "a".repeat(500);
    let mut ctx = TranslationContext::new();
    ctx.push(&long);
    let snap = ctx.snapshot();
    assert_eq!(snap[0].chars().count(), 300);
}

#[test]
fn translation_context_two_full_items_within_600_scalars() {
    let item: String = "z".repeat(300);
    let mut ctx = TranslationContext::new();
    ctx.push(&item);
    ctx.push(&item);
    let snap = ctx.snapshot();
    let total: usize = snap.iter().map(|s| s.chars().count()).sum();
    assert!(total <= 600);
}

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
