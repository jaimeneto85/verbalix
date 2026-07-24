use super::{
    macos_attribute,
    macos_ax::{self, AXUIElementRef, OwnedAxElement},
    macos_classic_range::{self, CFRange},
    macos_focus::{marker_fallback, AxCategory, AxFailure, AxStage, ExtractionOrigin},
    macos_geometry, macos_text_marker, macos_value_range,
};
use crate::domain::{
    GeometrySource, SelectionElementIdentity, SelectionExtractionStrategy, SelectionSnapshot,
    TextRange, VerbalixError,
};

struct ExtractedSelection {
    text: String,
    range: CFRange,
    bounds: crate::domain::Rect,
    geometry_source: GeometrySource,
    writable: bool,
    strategy: SelectionExtractionStrategy,
}

pub(super) fn capture(element: &OwnedAxElement) -> Result<SelectionSnapshot, VerbalixError> {
    let origin = ExtractionOrigin::SelectedText;
    let pid = validated_pid(element.as_ref(), origin)?;
    let role = role(element.as_ref())?;
    if role == "AXSecureTextField" {
        return Err(VerbalixError::ProtectedField);
    }
    let identity = element_identity(element.as_ref(), role.clone())?;
    let extracted = extract(element.as_ref(), &role)?;
    if validated_pid(element.as_ref(), strategy_origin(extracted.strategy))? != pid
        || element_identity(element.as_ref(), role)? != identity
    {
        return Err(VerbalixError::StaleSelection);
    }
    snapshot(pid, identity, extracted)
}

pub(super) fn capture_with_strategy(
    element: &OwnedAxElement,
    strategy: SelectionExtractionStrategy,
) -> Result<SelectionSnapshot, VerbalixError> {
    let origin = strategy_origin(strategy);
    let pid = validated_pid(element.as_ref(), origin)?;
    let role = role(element.as_ref())?;
    if role == "AXSecureTextField" {
        return Err(VerbalixError::ProtectedField);
    }
    let identity = element_identity(element.as_ref(), role.clone())?;
    let extracted = extract_for_strategy(element.as_ref(), &role, strategy)?;
    if validated_pid(element.as_ref(), origin)? != pid
        || element_identity(element.as_ref(), role)? != identity
    {
        return Err(VerbalixError::StaleSelection);
    }
    snapshot(pid, identity, extracted)
}

fn snapshot(
    pid: i32,
    identity: SelectionElementIdentity,
    extracted: ExtractedSelection,
) -> Result<SelectionSnapshot, VerbalixError> {
    if extracted.text.trim().is_empty() {
        return Err(VerbalixError::SelectionUnavailable);
    }
    if extracted.text.chars().count() > 12_000 {
        return Err(VerbalixError::TextTooLong);
    }
    Ok(SelectionSnapshot::new(
        pid,
        format!("pid:{pid}"),
        extracted.text,
        TextRange {
            location: extracted.range.location as i64,
            length: extracted.range.length as i64,
        },
        extracted.bounds,
        extracted.writable,
    )
    .with_geometry_source(extracted.geometry_source)
    .with_extraction_strategy(extracted.strategy)
    .with_element_identity(identity))
}

pub(super) fn element_identity(
    element: AXUIElementRef,
    role: String,
) -> Result<SelectionElementIdentity, VerbalixError> {
    let origin = ExtractionOrigin::SelectedText;
    let subrole = macos_ax::optional_string_attribute(element, "AXSubrole", AxStage::Role, origin)
        .map_err(|_| VerbalixError::StaleSelection)?;
    let identifier =
        macos_ax::optional_string_attribute(element, "AXIdentifier", AxStage::Role, origin)
            .map_err(|_| VerbalixError::StaleSelection)?;
    let frame = macos_geometry::element_frame(element).ok_or(VerbalixError::StaleSelection)?;
    Ok(SelectionElementIdentity {
        role,
        subrole,
        identifier,
        frame,
    })
}

pub(super) fn role(element: AXUIElementRef) -> Result<String, VerbalixError> {
    macos_ax::string_attribute(
        element,
        "AXRole",
        AxStage::Role,
        ExtractionOrigin::SelectedText,
    )
    .map_err(|_| VerbalixError::SelectionUnavailable)
}

fn extract(element: AXUIElementRef, role: &str) -> Result<ExtractedSelection, VerbalixError> {
    let direct = macos_ax::string_attribute(
        element,
        "AXSelectedText",
        AxStage::SelectedText,
        ExtractionOrigin::SelectedText,
    );
    match direct {
        Ok(text) => direct_selection(element, text),
        Err(failure) if marker_fallback(failure) => match cf_range_selection(element) {
            Ok(selection) => Ok(selection),
            Err(range_failure) if macos_value_range::fallback_eligible(range_failure) => {
                if !macos_value_range::role_eligible(role) {
                    return marker_selection(element);
                }
                match value_range_selection(element) {
                    Ok(selection) => Ok(selection),
                    Err(value_failure) if macos_value_range::marker_eligible(value_failure) => {
                        marker_selection(element)
                    }
                    Err(_) => Err(VerbalixError::SelectionUnavailable),
                }
            }
            Err(range_failure)
                if macos_classic_range::marker_eligible_after_range(range_failure) =>
            {
                marker_selection(element)
            }
            Err(_) => Err(VerbalixError::SelectionUnavailable),
        },
        Err(_) => Err(VerbalixError::SelectionUnavailable),
    }
}

fn extract_for_strategy(
    element: AXUIElementRef,
    role: &str,
    strategy: SelectionExtractionStrategy,
) -> Result<ExtractedSelection, VerbalixError> {
    match strategy {
        SelectionExtractionStrategy::SelectedText => {
            let text = macos_ax::string_attribute(
                element,
                "AXSelectedText",
                AxStage::SelectedText,
                ExtractionOrigin::SelectedText,
            )
            .map_err(|_| VerbalixError::StaleSelection)?;
            direct_selection(element, text)
        }
        SelectionExtractionStrategy::StringForRange => {
            cf_range_selection(element).map_err(|_| VerbalixError::StaleSelection)
        }
        SelectionExtractionStrategy::ValueRange if macos_value_range::role_eligible(role) => {
            value_range_selection(element).map_err(|_| VerbalixError::StaleSelection)
        }
        SelectionExtractionStrategy::ValueRange | SelectionExtractionStrategy::TextMarker => {
            Err(VerbalixError::StaleSelection)
        }
    }
}

fn direct_selection(
    element: AXUIElementRef,
    text: String,
) -> Result<ExtractedSelection, VerbalixError> {
    let range = macos_classic_range::selected_range(element)
        .map_err(|_| VerbalixError::SelectionUnavailable)?;
    let (bounds, geometry_source) = macos_geometry::resolve(element, range.location, range.length)
        .ok_or(VerbalixError::SelectionUnavailable)?;
    Ok(ExtractedSelection {
        text,
        range,
        bounds,
        geometry_source,
        writable: macos_attribute::selected_text_writable(element)
            .map_err(|_| VerbalixError::SelectionUnavailable)?,
        strategy: SelectionExtractionStrategy::SelectedText,
    })
}

fn cf_range_selection(element: AXUIElementRef) -> Result<ExtractedSelection, AxFailure> {
    let range = macos_classic_range::selected_range(element)?;
    let text = macos_classic_range::string_for_range(element, range)?;
    let (bounds, geometry_source) = macos_geometry::resolve(element, range.location, range.length)
        .ok_or_else(|| AxFailure::new(AxStage::Geometry, AxCategory::NoValue))?;
    Ok(ExtractedSelection {
        text,
        range,
        bounds,
        geometry_source,
        writable: macos_attribute::selected_text_writable(element)?,
        strategy: SelectionExtractionStrategy::StringForRange,
    })
}

fn value_range_selection(element: AXUIElementRef) -> Result<ExtractedSelection, AxFailure> {
    let selection = macos_value_range::extract(element)?;
    let (bounds, geometry_source) =
        macos_geometry::resolve(element, selection.range.location, selection.range.length)
            .ok_or_else(|| AxFailure::new(AxStage::Geometry, AxCategory::NoValue))?;
    Ok(ExtractedSelection {
        text: selection.text,
        range: selection.range,
        bounds,
        geometry_source,
        writable: macos_attribute::selected_text_writable(element)?,
        strategy: SelectionExtractionStrategy::ValueRange,
    })
}

fn marker_selection(element: AXUIElementRef) -> Result<ExtractedSelection, VerbalixError> {
    let marker =
        macos_text_marker::extract(element).map_err(|_| VerbalixError::SelectionUnavailable)?;
    Ok(ExtractedSelection {
        text: marker.text,
        range: marker.range,
        bounds: marker.bounds,
        geometry_source: GeometrySource::TextMarkerRange,
        writable: false,
        strategy: SelectionExtractionStrategy::TextMarker,
    })
}

fn strategy_origin(strategy: SelectionExtractionStrategy) -> ExtractionOrigin {
    match strategy {
        SelectionExtractionStrategy::SelectedText => ExtractionOrigin::SelectedText,
        SelectionExtractionStrategy::StringForRange => ExtractionOrigin::CfRange,
        SelectionExtractionStrategy::ValueRange => ExtractionOrigin::ValueRange,
        SelectionExtractionStrategy::TextMarker => ExtractionOrigin::TextMarker,
    }
}

fn validated_pid(element: AXUIElementRef, origin: ExtractionOrigin) -> Result<i32, VerbalixError> {
    let pid = macos_ax::pid(element).map_err(|failure| {
        crate::diagnostics::ax_resolution(AxStage::Pid, origin, failure.category);
        VerbalixError::SelectionUnavailable
    })?;
    let own_pid =
        i32::try_from(std::process::id()).map_err(|_| VerbalixError::SelectionUnavailable)?;
    if pid == own_pid {
        crate::diagnostics::ax_resolution(AxStage::Pid, origin, AxCategory::SelfProcess);
        return Err(VerbalixError::SelectionUnavailable);
    }
    crate::diagnostics::ax_resolution(AxStage::Pid, origin, AxCategory::Success);
    Ok(pid)
}
