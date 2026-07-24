import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import { bootstrapDocument, overlayKind } from "./overlaySurface";

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

      const resolved = bootstrapDocument(`?overlay=${kind}`, render);

      expect(resolved).toBe(kind);
      expect(render).toHaveBeenCalledOnce();
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

});
