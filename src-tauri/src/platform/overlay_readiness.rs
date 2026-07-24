use crate::domain::VerbalixError;
use std::{
    collections::{HashMap, HashSet},
    sync::{Mutex, MutexGuard},
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OverlaySurface {
    Toolbar,
    Note,
}

impl OverlaySurface {
    pub fn from_label(label: &str) -> Result<Self, VerbalixError> {
        match label {
            "toolbar" => Ok(Self::Toolbar),
            "note" => Ok(Self::Note),
            _ => Err(VerbalixError::LocalFailure),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Toolbar => "toolbar",
            Self::Note => "note",
        }
    }
}

#[derive(Default)]
pub struct OverlayReadiness {
    state: Mutex<SurfaceState>,
}

#[derive(Default)]
struct SurfaceState {
    current: HashMap<OverlaySurface, Uuid>,
    ready: HashMap<OverlaySurface, Uuid>,
    requested: HashSet<OverlaySurface>,
}

impl OverlayReadiness {
    pub fn has_document(&self, surface: OverlaySurface) -> Result<bool, VerbalixError> {
        Ok(self.lock()?.current.contains_key(&surface))
    }

    pub fn should_show(&self, surface: OverlaySurface) -> Result<bool, VerbalixError> {
        let state = self.lock()?;
        Ok(state.current.get(&surface) == state.ready.get(&surface)
            && state.current.contains_key(&surface)
            && state.requested.contains(&surface))
    }

    pub fn begin_document(&self, surface: OverlaySurface) -> Result<Uuid, VerbalixError> {
        let generation = Uuid::new_v4();
        let mut state = self.lock()?;
        state.current.insert(surface, generation);
        state.ready.remove(&surface);
        Ok(generation)
    }

    pub fn mark_ready(
        &self,
        surface: OverlaySurface,
        generation: Uuid,
    ) -> Result<bool, VerbalixError> {
        let mut state = self.lock()?;
        if state.current.get(&surface) != Some(&generation) {
            return Ok(false);
        }
        state.ready.insert(surface, generation);
        Ok(true)
    }

    pub fn invalidate_if_current(
        &self,
        surface: OverlaySurface,
        expected_generation: Uuid,
    ) -> Result<bool, VerbalixError> {
        let mut state = self.lock()?;
        if state.current.get(&surface) != Some(&expected_generation) {
            return Ok(false);
        }
        state.current.remove(&surface);
        state.ready.remove(&surface);
        Ok(true)
    }

    pub fn request(&self, surface: OverlaySurface) -> Result<(), VerbalixError> {
        self.lock()?.requested.insert(surface);
        Ok(())
    }

    pub fn cancel(&self, surface: OverlaySurface) -> Result<(), VerbalixError> {
        self.lock()?.requested.remove(&surface);
        Ok(())
    }

    pub fn cancel_all(&self) -> Result<(), VerbalixError> {
        self.lock()?.requested.clear();
        Ok(())
    }

    fn lock(&self) -> Result<MutexGuard<'_, SurfaceState>, VerbalixError> {
        self.state.lock().map_err(|_| VerbalixError::LocalFailure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_surface_stays_hidden_until_the_current_document_marks_it_ready() {
        let readiness = OverlayReadiness::default();
        let generation = readiness.begin_document(OverlaySurface::Toolbar).unwrap();

        readiness.request(OverlaySurface::Toolbar).unwrap();
        assert!(!readiness.should_show(OverlaySurface::Toolbar).unwrap());
        assert!(readiness
            .mark_ready(OverlaySurface::Toolbar, generation)
            .unwrap());
        assert!(readiness.should_show(OverlaySurface::Toolbar).unwrap());
        assert!(!readiness.should_show(OverlaySurface::Note).unwrap());
        readiness.begin_document(OverlaySurface::Toolbar).unwrap();
        assert!(!readiness.should_show(OverlaySurface::Toolbar).unwrap());
    }

    #[test]
    fn a_delayed_ready_signal_cannot_resurrect_a_hidden_surface() {
        let readiness = OverlayReadiness::default();
        let generation = readiness.begin_document(OverlaySurface::Toolbar).unwrap();
        readiness.request(OverlaySurface::Toolbar).unwrap();
        readiness.cancel_all().unwrap();

        readiness
            .mark_ready(OverlaySurface::Toolbar, generation)
            .unwrap();

        assert!(!readiness.should_show(OverlaySurface::Toolbar).unwrap());
    }

    #[test]
    fn an_old_ack_cannot_mark_or_show_a_recreated_document() {
        let readiness = OverlayReadiness::default();
        let old_generation = readiness.begin_document(OverlaySurface::Toolbar).unwrap();
        readiness.request(OverlaySurface::Toolbar).unwrap();
        readiness.cancel_all().unwrap();
        let current_generation = readiness.begin_document(OverlaySurface::Toolbar).unwrap();
        readiness.request(OverlaySurface::Toolbar).unwrap();

        assert!(!readiness
            .mark_ready(OverlaySurface::Toolbar, old_generation)
            .unwrap());
        assert!(!readiness.should_show(OverlaySurface::Toolbar).unwrap());
        assert!(readiness
            .mark_ready(OverlaySurface::Toolbar, current_generation)
            .unwrap());
        assert!(readiness.should_show(OverlaySurface::Toolbar).unwrap());
    }

    #[test]
    fn stale_invalidation_preserves_the_current_ready_generation() {
        let readiness = OverlayReadiness::default();
        let surface = OverlaySurface::Toolbar;
        let old_generation = readiness.begin_document(surface).unwrap();
        let current_generation = readiness.begin_document(surface).unwrap();
        readiness.request(surface).unwrap();
        assert!(readiness.mark_ready(surface, current_generation).unwrap());

        assert!(!readiness
            .invalidate_if_current(surface, old_generation)
            .unwrap());

        assert!(readiness.has_document(surface).unwrap());
        assert!(readiness.should_show(surface).unwrap());
        assert!(readiness.mark_ready(surface, current_generation).unwrap());
    }

    #[test]
    fn unsupported_labels_fail_closed() {
        assert!(OverlaySurface::from_label("toolbar").is_ok());
        assert!(OverlaySurface::from_label("note").is_ok());
        assert!(OverlaySurface::from_label("main").is_err());
    }
}
