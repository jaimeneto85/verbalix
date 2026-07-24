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
  const uuid = generation?.trim() ?? "";
  return /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(
    uuid
  )
    ? uuid
    : null;
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
