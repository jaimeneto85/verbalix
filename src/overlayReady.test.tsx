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
      <OverlayReadyGate kind="toolbar" acknowledge={acknowledge}>
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

    const acknowledged = await acknowledgeOverlayReady("note", {
      send,
      wait,
      report,
      attempts: 4,
      timeoutMs: 50
    });

    expect(acknowledged).toBe(true);
    expect(send).toHaveBeenCalledTimes(3);
    expect(wait).toHaveBeenCalledTimes(5);
    expect(report).not.toHaveBeenCalled();
  });

  it("bounds retries, reports failure and stops without an ack", async () => {
    const send = vi.fn(() => new Promise<boolean>(() => undefined));
    const wait = vi.fn(async () => undefined);
    const report = vi.fn();

    const acknowledged = await acknowledgeOverlayReady("toolbar", {
      send,
      wait,
      report,
      attempts: 3,
      timeoutMs: 50
    });

    expect(acknowledged).toBe(false);
    expect(send).toHaveBeenCalledTimes(3);
    expect(wait).toHaveBeenCalledTimes(5);
    expect(report).toHaveBeenCalledOnce();
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
      <OverlayReadyGate kind="toolbar" acknowledge={acknowledge}>
        <CommittedChild />
      </OverlayReadyGate>
    );

    await waitFor(() => expect(acknowledge).toHaveBeenCalledOnce());
    expect(order).toEqual(["child", "gate"]);
  });
});
