use super::{
    macos_ax::OwnedAxElement,
    macos_replace::{self, WriteOutcome},
    macos_restore::{self, RestoreWriteOutcome},
    macos_selection_revalidation::{self, CurrentSelection},
    macos_write_authorization::AxWriteAuthorization,
};
use crate::domain::{SelectionExtractionStrategy, SelectionSnapshot, VerbalixError};
use std::rc::Rc;

pub(super) trait AxMutationTarget {
    fn prepare_replace(
        &self,
        expected: &SelectionSnapshot,
        causal: bool,
    ) -> Result<(), VerbalixError>;

    fn write_replace(
        &self,
        expected: &SelectionSnapshot,
        text: &str,
        authorization: &AxWriteAuthorization,
    ) -> WriteOutcome;

    fn prepare_restore(
        &self,
        expected: &SelectionSnapshot,
        transformed: &str,
        causal: bool,
    ) -> Result<(), VerbalixError>;

    fn write_restore(
        &self,
        expected: &SelectionSnapshot,
        authorization: &AxWriteAuthorization,
    ) -> RestoreWriteOutcome;

    fn read(
        &self,
        strategy: SelectionExtractionStrategy,
    ) -> Result<CurrentSelection, VerbalixError>;
}

struct NativeAxMutationTarget {
    element: Rc<OwnedAxElement>,
}

impl AxMutationTarget for NativeAxMutationTarget {
    fn prepare_replace(
        &self,
        expected: &SelectionSnapshot,
        causal: bool,
    ) -> Result<(), VerbalixError> {
        macos_replace::prepare_on_element(expected, &self.element, causal)
    }

    fn write_replace(
        &self,
        expected: &SelectionSnapshot,
        text: &str,
        authorization: &AxWriteAuthorization,
    ) -> WriteOutcome {
        macos_replace::write_on_element(
            expected,
            text,
            self.element.as_ref().as_ref(),
            authorization,
        )
    }

    fn prepare_restore(
        &self,
        expected: &SelectionSnapshot,
        transformed: &str,
        causal: bool,
    ) -> Result<(), VerbalixError> {
        macos_restore::prepare_on_element(expected, transformed, &self.element, causal)
    }

    fn write_restore(
        &self,
        expected: &SelectionSnapshot,
        authorization: &AxWriteAuthorization,
    ) -> RestoreWriteOutcome {
        macos_restore::write_on_element(expected, self.element.as_ref().as_ref(), authorization)
    }

    fn read(
        &self,
        strategy: SelectionExtractionStrategy,
    ) -> Result<CurrentSelection, VerbalixError> {
        macos_selection_revalidation::read(self.element.as_ref().as_ref(), strategy)
    }
}

pub(super) fn native_target(element: Rc<OwnedAxElement>) -> Rc<dyn AxMutationTarget> {
    Rc::new(NativeAxMutationTarget { element })
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::{
        domain::VerbalixError,
        platform::{
            macos_ax::AxWriteResult, macos_text_role,
            macos_write_boundary::set_after_role_validation,
        },
    };
    use std::cell::{Cell, RefCell};

    pub(crate) struct InstrumentedAxTarget {
        secure_after_prepare: Cell<bool>,
        role: RefCell<(String, Option<String>)>,
        setters: Cell<usize>,
        prepares: Cell<usize>,
    }

    impl InstrumentedAxTarget {
        pub(crate) fn secure_after_prepare() -> Rc<Self> {
            Rc::new(Self {
                secure_after_prepare: Cell::new(true),
                role: RefCell::new(("AXTextField".to_owned(), None)),
                setters: Cell::new(0),
                prepares: Cell::new(0),
            })
        }

        pub(crate) fn setters(&self) -> usize {
            self.setters.get()
        }

        pub(crate) fn prepares(&self) -> usize {
            self.prepares.get()
        }

        fn prepare(&self) {
            self.prepares.set(self.prepares.get() + 1);
            if self.secure_after_prepare.get() {
                self.role.replace((
                    "AXTextField".to_owned(),
                    Some("AXSecureTextField".to_owned()),
                ));
            }
        }

        fn write(
            &self,
            expected: &SelectionSnapshot,
            authorization: &AxWriteAuthorization,
        ) -> bool {
            let (role, subrole) = self.role.borrow().clone();
            let validated = macos_text_role::validate(role, subrole);
            validated
                .and_then(|current| {
                    set_after_role_validation(
                        expected.element_identity.as_ref().unwrap(),
                        current,
                        authorization,
                        || {
                            self.setters.set(self.setters.get() + 1);
                            AxWriteResult::Confirmed
                        },
                    )
                })
                .is_ok()
        }
    }

    impl AxMutationTarget for InstrumentedAxTarget {
        fn prepare_replace(
            &self,
            _expected: &SelectionSnapshot,
            _causal: bool,
        ) -> Result<(), VerbalixError> {
            self.prepare();
            Ok(())
        }

        fn write_replace(
            &self,
            expected: &SelectionSnapshot,
            _text: &str,
            authorization: &AxWriteAuthorization,
        ) -> WriteOutcome {
            if self.write(expected, authorization) {
                WriteOutcome::Confirmed
            } else {
                WriteOutcome::Rejected
            }
        }

        fn prepare_restore(
            &self,
            _expected: &SelectionSnapshot,
            _transformed: &str,
            _causal: bool,
        ) -> Result<(), VerbalixError> {
            self.prepare();
            Ok(())
        }

        fn write_restore(
            &self,
            expected: &SelectionSnapshot,
            authorization: &AxWriteAuthorization,
        ) -> RestoreWriteOutcome {
            if self.write(expected, authorization) {
                RestoreWriteOutcome::Confirmed
            } else {
                RestoreWriteOutcome::Rejected
            }
        }

        fn read(
            &self,
            _strategy: SelectionExtractionStrategy,
        ) -> Result<CurrentSelection, VerbalixError> {
            Err(VerbalixError::ProtectedField)
        }
    }
}
