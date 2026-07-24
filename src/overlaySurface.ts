export type OverlayKind = "toolbar" | "note";

type RenderOverlay = (root: HTMLElement, overlay: OverlayKind | null) => void;
type SurfaceReady = (overlay: OverlayKind) => void;

export function overlayKind(search: string): OverlayKind | null {
  const candidate = new URLSearchParams(search).get("overlay");
  return candidate === "toolbar" || candidate === "note" ? candidate : null;
}

export function bootstrapDocument(
  search: string,
  render: RenderOverlay,
  surfaceReady: SurfaceReady = () => undefined,
  target: Document = document
): OverlayKind | null {
  const overlay = overlayKind(search);
  target.documentElement.classList.toggle("overlay-surface", overlay !== null);
  const root = target.getElementById("root");
  if (!root) {
    throw new Error("Missing application root");
  }
  render(root, overlay);
  if (overlay) {
    surfaceReady(overlay);
  }
  return overlay;
}
