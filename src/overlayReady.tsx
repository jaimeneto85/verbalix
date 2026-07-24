import {
  type PropsWithChildren,
  useLayoutEffect,
  useRef
} from "react";
import { native } from "./native";
import type { OverlayKind } from "./overlaySurface";

type ReadyClient = (
  kind: OverlayKind,
  generation: string
) => Promise<boolean>;
type Wait = (milliseconds: number) => Promise<void>;
type Report = (error: unknown) => void;

type ReadyOptions = {
  send?: ReadyClient;
  wait?: Wait;
  report?: Report;
  attempts?: number;
};

const waitFor = (milliseconds: number) =>
  new Promise<void>((resolve) => window.setTimeout(resolve, milliseconds));

const reportFailure = (error: unknown) => {
  console.error("Overlay readiness handshake failed", error);
};

export async function acknowledgeOverlayReady(
  kind: OverlayKind,
  generation: string,
  options: ReadyOptions = {}
): Promise<boolean> {
  const send = options.send ?? native.overlaySurfaceReady;
  const wait = options.wait ?? waitFor;
  const report = options.report ?? reportFailure;
  const attempts = Math.min(3, Math.max(1, options.attempts ?? 3));
  let lastError: unknown = new Error("Overlay readiness was not acknowledged");

  for (let attempt = 0; attempt < attempts; attempt += 1) {
    try {
      const acknowledged = await send(kind, generation);
      if (acknowledged) {
        return true;
      }
      lastError = new Error("Overlay readiness was not acknowledged");
    } catch (error) {
      lastError = error;
    }
    if (attempt + 1 < attempts) {
      await wait(50 * (attempt + 1));
    }
  }

  report(lastError);
  return false;
}

type GateProps = PropsWithChildren<{
  kind: OverlayKind;
  generation: string;
  acknowledge?: ReadyClient;
}>;

export function OverlayReadyGate({
  kind,
  generation,
  acknowledge = acknowledgeOverlayReady,
  children
}: GateProps) {
  const started = useRef(false);

  useLayoutEffect(() => {
    if (started.current) {
      return;
    }
    started.current = true;
    let active = true;
    void acknowledge(kind, generation).then((acknowledged) => {
      if (active) {
        document.documentElement.dataset.overlayReadiness = acknowledged
          ? "acknowledged"
          : "failed";
      }
    });
    return () => {
      active = false;
    };
  }, [acknowledge, generation, kind]);

  return children;
}
