export type OverlayKind = "toolbar" | "note";

type RenderOverlay = (
  root: HTMLElement,
  overlay: OverlayKind | null,
  generation: string | null
) => void;

export function overlayKind(search: string): OverlayKind | null {
  const candidate = new URLSearchParams(search).get("overlay");
  return candidate === "toolbar" || candidate === "note" ? candidate : null;
}

export function overlayGeneration(search: string): string | null {
  const generation = new URLSearchParams(search).get("generation");
  return generation?.trim() || null;
}

export function bootstrapDocument(
  search: string,
  render: RenderOverlay,
  target: Document = document
): OverlayKind | null {
  const overlay = overlayKind(search);
  const generation = overlayGeneration(search);
  target.documentElement.classList.toggle("overlay-surface", overlay !== null);
  const root = target.getElementById("root");
  if (!root) {
    throw new Error("Missing application root");
  }
  render(root, overlay, generation);
  return overlay;
}
