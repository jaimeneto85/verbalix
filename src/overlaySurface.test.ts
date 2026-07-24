import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  bootstrapDocument,
  overlayGeneration,
  overlayKind
} from "./overlaySurface";

const overlayCss = readFileSync("src/styles/base.css", "utf8");

function installStyles() {
  const style = document.createElement("style");
  style.textContent = overlayCss;
  document.head.append(style);
}

afterEach(() => {
  document.documentElement.className = "";
  document.head.innerHTML = "";
  document.body.innerHTML = "";
});

describe("overlay document surface", () => {
  it.each(["toolbar", "note"] as const)(
    "marks %s as transparent before rendering",
    (kind) => {
      document.body.innerHTML = '<div id="root"></div>';
      installStyles();
      const render = vi.fn(() => {
        expect(document.documentElement).toHaveClass("overlay-surface");
      });

      const resolved = bootstrapDocument(
        `?overlay=${kind}&generation=123e4567-e89b-42d3-a456-426614174000`,
        render
      );

      expect(resolved).toBe(kind);
      expect(render).toHaveBeenCalledOnce();
      expect(render).toHaveBeenCalledWith(
        document.getElementById("root"),
        kind,
        "123e4567-e89b-42d3-a456-426614174000"
      );
      for (const element of [
        document.documentElement,
        document.body,
        document.getElementById("root")!
      ]) {
        const styles = getComputedStyle(element);
        expect(styles.backgroundColor).toBe("rgba(0, 0, 0, 0)");
        expect(Number.parseFloat(styles.minWidth)).toBe(0);
        expect(Number.parseFloat(styles.minHeight)).toBe(0);
        expect(styles.overflow).toBe("hidden");
      }
    }
  );

  it.each(["", "?overlay=unknown"])(
    "preserves the main surface for search %s",
    (search) => {
      document.body.innerHTML = '<div id="root"></div>';
      installStyles();

      const resolved = bootstrapDocument(search, vi.fn());

      expect(resolved).toBeNull();
      expect(document.documentElement).not.toHaveClass("overlay-surface");
      expect(getComputedStyle(document.documentElement).backgroundColor).toBe(
        "rgb(238, 241, 245)"
      );
      expect(getComputedStyle(document.body).minWidth).toBe("320px");
    }
  );

  it("recognizes only supported overlay routes", () => {
    expect(overlayKind("?overlay=toolbar")).toBe("toolbar");
    expect(overlayKind("?overlay=note")).toBe("note");
    expect(overlayKind("?overlay=")).toBeNull();
    expect(overlayKind("?overlay=settings")).toBeNull();
  });

  it("reads the Rust-issued document generation from the overlay URL", () => {
    expect(
      overlayGeneration(
        "?generation=123e4567-e89b-42d3-a456-426614174000"
      )
    ).toBe("123e4567-e89b-42d3-a456-426614174000");
    expect(overlayGeneration("?generation=test-generation")).toBeNull();
    expect(
      overlayGeneration(
        "?generation=123e4567-e89b-12d3-a456-426614174000"
      )
    ).toBeNull();
    expect(overlayGeneration("?generation=")).toBeNull();
    expect(overlayGeneration("")).toBeNull();
  });
});
