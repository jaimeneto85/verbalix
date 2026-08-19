use std::{
    collections::VecDeque,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

pub struct StreamSegmentHandle {
    pub buffer: Arc<Mutex<VecDeque<f32>>>,
    pub complete: Arc<AtomicBool>,
    pub cancel: Arc<AtomicBool>,
    pub detected_language: String,
    pub target_language: String,
    pub stt_ms: u32,
    pub translate_ms: u32,
    pub source_text: String,
}
