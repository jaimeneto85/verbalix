use crate::domain::VerbalixError;
use std::{
    collections::HashSet,
    sync::{Mutex, MutexGuard},
};

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
    ready: HashSet<OverlaySurface>,
    requested: HashSet<OverlaySurface>,
}

impl OverlayReadiness {
    pub fn should_show(&self, surface: OverlaySurface) -> Result<bool, VerbalixError> {
        let state = self.lock()?;
        Ok(state.ready.contains(&surface) && state.requested.contains(&surface))
    }

    pub fn mark_ready(&self, surface: OverlaySurface) -> Result<(), VerbalixError> {
        self.lock()?.ready.insert(surface);
        Ok(())
    }

    pub fn clear_ready(&self, surface: OverlaySurface) -> Result<(), VerbalixError> {
        self.lock()?.ready.remove(&surface);
        Ok(())
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

        readiness.request(OverlaySurface::Toolbar).unwrap();
        assert!(!readiness.should_show(OverlaySurface::Toolbar).unwrap());
        readiness.mark_ready(OverlaySurface::Toolbar).unwrap();
        assert!(readiness.should_show(OverlaySurface::Toolbar).unwrap());
        assert!(!readiness.should_show(OverlaySurface::Note).unwrap());
        readiness.clear_ready(OverlaySurface::Toolbar).unwrap();
        assert!(!readiness.should_show(OverlaySurface::Toolbar).unwrap());
    }

    #[test]
    fn a_delayed_ready_signal_cannot_resurrect_a_hidden_surface() {
        let readiness = OverlayReadiness::default();
        readiness.request(OverlaySurface::Toolbar).unwrap();
        readiness.cancel_all().unwrap();

        readiness.mark_ready(OverlaySurface::Toolbar).unwrap();

        assert!(!readiness.should_show(OverlaySurface::Toolbar).unwrap());
    }

    #[test]
    fn unsupported_labels_fail_closed() {
        assert!(OverlaySurface::from_label("toolbar").is_ok());
        assert!(OverlaySurface::from_label("note").is_ok());
        assert!(OverlaySurface::from_label("main").is_err());
    }
}
