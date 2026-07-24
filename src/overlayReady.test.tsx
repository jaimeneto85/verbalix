import { render, screen, waitFor } from "@testing-library/react";
import { useLayoutEffect } from "react";
import { describe, expect, it, vi } from "vitest";
import {
  acknowledgeOverlayReady,
  OverlayReadyGate
} from "./overlayReady";

describe("overlay readiness handshake", () => {
  it("does not acknowledge readiness before the overlay commits", async () => {
    const acknowledge = vi.fn(async () => {
      expect(screen.getByTestId("committed-overlay")).toBeInTheDocument();
      return true;
    });

    render(
      <OverlayReadyGate
        kind="toolbar"
        generation="toolbar-generation"
        acknowledge={acknowledge}
      >
        <div data-testid="committed-overlay" />
      </OverlayReadyGate>
    );

    await waitFor(() => expect(acknowledge).toHaveBeenCalledOnce());
  });

  it("retries transient failures and stops immediately after an ack", async () => {
    const send = vi
      .fn()
      .mockRejectedValueOnce(new Error("bridge unavailable"))
      .mockResolvedValueOnce(false)
      .mockResolvedValueOnce(true);
    const wait = vi.fn(async () => undefined);
    const report = vi.fn();

    const acknowledged = await acknowledgeOverlayReady("note", "note-generation", {
      send,
      wait,
      report,
      attempts: 8
    });

    expect(acknowledged).toBe(true);
    expect(send).toHaveBeenCalledTimes(3);
    expect(wait).toHaveBeenCalledTimes(2);
    expect(report).not.toHaveBeenCalled();
  });

  it("bounds retries, reports failure and stops without an ack", async () => {
    const send = vi.fn(async () => false);
    const wait = vi.fn(async () => undefined);
    const report = vi.fn();

    const acknowledged = await acknowledgeOverlayReady(
      "toolbar",
      "toolbar-generation",
      {
        send,
        wait,
        report,
        attempts: 30
      }
    );

    expect(acknowledged).toBe(false);
    expect(send).toHaveBeenCalledTimes(3);
    expect(wait).toHaveBeenCalledTimes(2);
    expect(report).toHaveBeenCalledOnce();
  });

  it("never starts a second invoke before the previous invoke settles", async () => {
    let settleFirst: ((acknowledged: boolean) => void) | undefined;
    const first = new Promise<boolean>((resolve) => {
      settleFirst = resolve;
    });
    const send = vi
      .fn()
      .mockReturnValueOnce(first)
      .mockResolvedValueOnce(true);
    const wait = vi.fn(async () => undefined);

    const handshake = acknowledgeOverlayReady(
      "toolbar",
      "current-generation",
      { send, wait }
    );
    await Promise.resolve();
    expect(send).toHaveBeenCalledTimes(1);

    settleFirst?.(false);
    await expect(handshake).resolves.toBe(true);
    expect(send).toHaveBeenCalledTimes(2);
    expect(send.mock.invocationCallOrder[0]).toBeLessThan(
      send.mock.invocationCallOrder[1]
    );
  });

  it("runs the gate from a layout effect after child DOM mutation", async () => {
    const order: string[] = [];
    function CommittedChild() {
      useLayoutEffect(() => {
        order.push("child");
      }, []);
      return <div />;
    }
    const acknowledge = vi.fn(async () => {
      order.push("gate");
      return true;
    });

    render(
      <OverlayReadyGate
        kind="toolbar"
        generation="toolbar-generation"
        acknowledge={acknowledge}
      >
        <CommittedChild />
      </OverlayReadyGate>
    );

    await waitFor(() => expect(acknowledge).toHaveBeenCalledOnce());
    expect(order).toEqual(["child", "gate"]);
  });
});
