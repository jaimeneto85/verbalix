import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = (path: string) => readFileSync(`src-tauri/src/platform/${path}`, "utf8");

describe("macOS value-range privacy and write contract", () => {
  it("reads two stable ranges before validating or copying selected UTF-16 units", () => {
    const valueRange = source("macos_value_range.rs");
    const extract = valueRange.slice(
      valueRange.indexOf("pub(super) fn extract"),
      valueRange.indexOf("pub(super) fn fallback_eligible")
    );
    const firstRange = extract.indexOf("let first");
    const value = extract.indexOf('"AXValue"', firstRange);
    const secondRange = extract.indexOf("let second", value);
    const validation = extract.indexOf("validate_value", secondRange);
    const copy = extract.indexOf("copy_selected_range", validation);

    expect(firstRange).toBeGreaterThan(-1);
    expect(value).toBeGreaterThan(firstRange);
    expect(secondRange).toBeGreaterThan(value);
    expect(validation).toBeGreaterThan(secondRange);
    expect(copy).toBeGreaterThan(validation);
  });

  it("copies only the selected CFString range and never writes AXValue", () => {
    const valueRange = source("macos_value_range.rs");
    const ax = source("macos_ax.rs");

    expect(valueRange).toContain("MAX_VALUE_UTF16_UNITS");
    expect(valueRange).toContain("CFStringGetCharacters");
    expect(valueRange).toContain("String::from_utf16");
    expect(valueRange).not.toContain("string_value(");
    expect(valueRange).not.toContain(".to_string()");
    expect(ax).not.toMatch(/set_[a-z_]*value/);
    expect(ax).toContain('CFString::new("AXSelectedText")');
  });

  it("checks protected roles before extraction and keeps strategy-bound writes", () => {
    const selection = source("macos_selection.rs");
    const textRole = source("macos_text_role.rs");
    const replace = source("macos_replace.rs");
    const restore = source("macos_restore.rs");
    const capture = selection.slice(
      selection.indexOf("pub(super) fn capture("),
      selection.indexOf("pub(super) fn capture_with_strategy")
    );

    const roleRead = capture.indexOf("let validated_role = text_role(element.as_ref())?");
    const identityRead = capture.indexOf("element_identity(element.as_ref()");
    const extraction = capture.indexOf("let extracted = extract(");
    expect(roleRead).toBeGreaterThan(-1);
    expect(identityRead).toBeGreaterThan(roleRead);
    expect(extraction).toBeGreaterThan(identityRead);
    expect(textRole).toContain('role == "AXSecureTextField"');
    expect(textRole).toContain(
      'subrole.as_deref() == Some("AXSecureTextField")'
    );
    expect(textRole).toContain('"AXButton"');
    const roleBoundary = selection.slice(
      selection.indexOf("pub(super) fn text_role("),
      selection.indexOf("fn extract(")
    );
    expect(roleBoundary.indexOf('"AXRole"')).toBeLessThan(
      roleBoundary.indexOf('"AXSubrole"')
    );
    expect(roleBoundary.indexOf('"AXSubrole"')).toBeLessThan(
      roleBoundary.indexOf("macos_text_role::validate(role, subrole)")
    );
    for (const forbidden of [
      "AXIdentifier",
      "selected_text_writable",
      "AXSelectedText",
      "AXSelectedTextRange",
      "AXValue",
      "marker_selection",
      "element_frame",
    ]) {
      expect(roleBoundary).not.toContain(forbidden);
    }
    const valueFallback = selection.slice(
      selection.indexOf("fn extract("),
      selection.indexOf("fn extract_for_strategy")
    );
    expect(valueFallback.indexOf("role_eligible(role)")).toBeLessThan(
      valueFallback.indexOf("value_range_selection(element)")
    );
    expect(replace).toContain(
      "capture_with_strategy(element, expected.extraction_strategy)"
    );
    expect(restore).toContain(
      "read(element.as_ref(), expected.extraction_strategy)"
    );
    expect(restore).toContain(
      "current.strategy != expected.extraction_strategy"
    );
    expect(restore.indexOf("macos_selection::text_role")).toBeLessThan(
      restore.indexOf("macos_selection::element_identity")
    );
    expect(restore.indexOf("macos_selection::text_role")).toBeLessThan(
      restore.indexOf("selected_text_writable")
    );
  });
});
