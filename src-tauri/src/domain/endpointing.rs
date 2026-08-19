pub struct EndpointerConfig {
    pub voice_threshold: f32,
    pub min_voiced_frames: u32,
    pub silence_close_frames: u32,
    pub max_frames: u32,
}

impl Default for EndpointerConfig {
    fn default() -> Self {
        Self {
            voice_threshold: 0.02,
            min_voiced_frames: 20,
            silence_close_frames: 35,
            max_frames: 750,
        }
    }
}

enum EndpointerState {
    Idle,
    Open,
}

pub enum EndpointEvent {
    Opened,
    Closed,
    Dropped,
    MaxDurationReached,
}

pub struct Endpointer {
    config: EndpointerConfig,
    state: EndpointerState,
    voiced_frames: u32,
    silent_frames: u32,
    total_open_frames: u32,
}

impl Endpointer {
    pub fn new(config: EndpointerConfig) -> Self {
        Self {
            config,
            state: EndpointerState::Idle,
            voiced_frames: 0,
            silent_frames: 0,
            total_open_frames: 0,
        }
    }

    pub fn push_frame(&mut self, rms: f32) -> Option<EndpointEvent> {
        match self.state {
            EndpointerState::Idle => {
                if rms > self.config.voice_threshold {
                    self.voiced_frames += 1;
                    if self.voiced_frames >= self.config.min_voiced_frames {
                        self.state = EndpointerState::Open;
                        self.silent_frames = 0;
                        self.total_open_frames = self.voiced_frames;
                        return Some(EndpointEvent::Opened);
                    }
                    None
                } else if self.voiced_frames > 0 {
                    self.voiced_frames = 0;
                    Some(EndpointEvent::Dropped)
                } else {
                    None
                }
            }
            EndpointerState::Open => {
                self.total_open_frames += 1;

                if self.total_open_frames >= self.config.max_frames {
                    self.reset();
                    return Some(EndpointEvent::MaxDurationReached);
                }

                if rms > self.config.voice_threshold {
                    self.silent_frames = 0;
                } else {
                    self.silent_frames += 1;
                    if self.silent_frames > self.config.silence_close_frames {
                        self.reset();
                        return Some(EndpointEvent::Closed);
                    }
                }
                None
            }
        }
    }

    pub fn reset(&mut self) {
        self.state = EndpointerState::Idle;
        self.voiced_frames = 0;
        self.silent_frames = 0;
        self.total_open_frames = 0;
    }

    pub fn is_open(&self) -> bool {
        matches!(self.state, EndpointerState::Open)
    }
}

#[cfg(test)]
#[path = "endpointing_tests.rs"]
mod tests;
