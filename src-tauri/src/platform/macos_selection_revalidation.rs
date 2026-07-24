use super::{
    macos_ax::{self, AXUIElementRef},
    macos_classic_range::{self, CFRange},
    macos_focus::{AxStage, ExtractionOrigin},
    macos_value_range,
};
use crate::domain::{SelectionExtractionStrategy, VerbalixError};

pub(super) struct CurrentSelection {
    pub(super) text: String,
    pub(super) range: CFRange,
    pub(super) strategy: SelectionExtractionStrategy,
}

pub(super) fn read(
    element: AXUIElementRef,
    strategy: SelectionExtractionStrategy,
) -> Result<CurrentSelection, VerbalixError> {
    match strategy {
        SelectionExtractionStrategy::SelectedText => selected_text(element, strategy),
        SelectionExtractionStrategy::StringForRange => string_for_range(element, strategy),
        SelectionExtractionStrategy::ValueRange => value_range(element, strategy),
        SelectionExtractionStrategy::TextMarker => Err(VerbalixError::StaleSelection),
    }
}

fn selected_text(
    element: AXUIElementRef,
    strategy: SelectionExtractionStrategy,
) -> Result<CurrentSelection, VerbalixError> {
    let text = macos_ax::string_attribute(
        element,
        "AXSelectedText",
        AxStage::SelectedText,
        ExtractionOrigin::SelectedText,
    )
    .map_err(|_| VerbalixError::StaleSelection)?;
    let range =
        macos_classic_range::selected_range(element).map_err(|_| VerbalixError::StaleSelection)?;
    Ok(CurrentSelection {
        text,
        range,
        strategy,
    })
}

fn string_for_range(
    element: AXUIElementRef,
    strategy: SelectionExtractionStrategy,
) -> Result<CurrentSelection, VerbalixError> {
    let range =
        macos_classic_range::selected_range(element).map_err(|_| VerbalixError::StaleSelection)?;
    let text = macos_classic_range::string_for_range(element, range)
        .map_err(|_| VerbalixError::StaleSelection)?;
    Ok(CurrentSelection {
        text,
        range,
        strategy,
    })
}

fn value_range(
    element: AXUIElementRef,
    strategy: SelectionExtractionStrategy,
) -> Result<CurrentSelection, VerbalixError> {
    let selection =
        macos_value_range::extract(element).map_err(|_| VerbalixError::StaleSelection)?;
    Ok(CurrentSelection {
        text: selection.text,
        range: selection.range,
        strategy,
    })
}
