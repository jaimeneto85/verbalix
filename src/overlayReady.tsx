import {
  type PropsWithChildren,
  useLayoutEffect,
  useRef
} from "react";
import { native } from "./native";
import type { OverlayKind } from "./overlaySurface";

type ReadyClient = (kind: OverlayKind) => Promise<boolean>;
type Wait = (milliseconds: number) => Promise<void>;
type Report = (error: unknown) => void;

type ReadyOptions = {
  send?: ReadyClient;
  wait?: Wait;
  report?: Report;
  attempts?: number;
  timeoutMs?: number;
};

const waitFor = (milliseconds: number) =>
  new Promise<void>((resolve) => window.setTimeout(resolve, milliseconds));

const reportFailure = (error: unknown) => {
  console.error("Overlay readiness handshake failed", error);
};

export async function acknowledgeOverlayReady(
  kind: OverlayKind,
  options: ReadyOptions = {}
): Promise<boolean> {
  const send = options.send ?? native.overlaySurfaceReady;
  const wait = options.wait ?? waitFor;
  const report = options.report ?? reportFailure;
  const attempts = Math.max(1, options.attempts ?? 3);
  const timeoutMs = options.timeoutMs ?? 500;
  let lastError: unknown = new Error("Overlay readiness was not acknowledged");

  for (let attempt = 0; attempt < attempts; attempt += 1) {
    try {
      const acknowledged = await Promise.race([
        send(kind),
        wait(timeoutMs).then(() => false)
      ]);
      if (acknowledged) {
        return true;
      }
      lastError = new Error("Overlay readiness acknowledgment timed out");
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
  acknowledge?: ReadyClient;
}>;

export function OverlayReadyGate({
  kind,
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
    void acknowledge(kind).then((acknowledged) => {
      if (active) {
        document.documentElement.dataset.overlayReadiness = acknowledged
          ? "acknowledged"
          : "failed";
      }
    });
    return () => {
      active = false;
    };
  }, [acknowledge, kind]);

  return children;
}
