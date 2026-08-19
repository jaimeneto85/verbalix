import { assertEquals } from "jsr:@std/assert";
import {
  buildMetadataFrame,
  bodyWithInactivityWatchdog,
  prependFrame,
  METADATA_MAGIC,
} from "./streaming.ts";

Deno.test("buildMetadataFrame - starts with VLBX magic bytes", () => {
  const frame = buildMetadataFrame("test");
  assertEquals(frame[0], METADATA_MAGIC[0]);
  assertEquals(frame[1], METADATA_MAGIC[1]);
  assertEquals(frame[2], METADATA_MAGIC[2]);
  assertEquals(frame[3], METADATA_MAGIC[3]);
});

Deno.test("buildMetadataFrame - u32 big-endian length matches embedded JSON", () => {
  const frame = buildMetadataFrame("hello");
  const jsonLen = new DataView(frame.buffer).getUint32(4, false);
  const jsonBytes = frame.slice(8, 8 + jsonLen);
  const parsed = JSON.parse(new TextDecoder().decode(jsonBytes)) as { sourceText: string };
  assertEquals(parsed.sourceText, "hello");
  assertEquals(frame.byteLength, 8 + jsonLen);
});

Deno.test("buildMetadataFrame - sourceText capped to 300 scalars", () => {
  const longText = "あ".repeat(350);
  const frame = buildMetadataFrame(longText);
  const jsonLen = new DataView(frame.buffer).getUint32(4, false);
  const jsonBytes = frame.slice(8, 8 + jsonLen);
  const parsed = JSON.parse(new TextDecoder().decode(jsonBytes)) as { sourceText: string };
  assertEquals([...parsed.sourceText].length, 300);
});

Deno.test("buildMetadataFrame - empty sourceText produces valid frame", () => {
  const frame = buildMetadataFrame("");
  const jsonLen = new DataView(frame.buffer).getUint32(4, false);
  const jsonBytes = frame.slice(8, 8 + jsonLen);
  const parsed = JSON.parse(new TextDecoder().decode(jsonBytes)) as { sourceText: string };
  assertEquals(parsed.sourceText, "");
});

Deno.test("prependFrame - emits frame bytes first then source stream bytes", async () => {
  const frame = new Uint8Array([1, 2, 3]);
  const sourceBytes = new Uint8Array([4, 5, 6]);
  const source = new ReadableStream<Uint8Array>({
    start(c) { c.enqueue(sourceBytes); c.close(); },
  });

  const combined = prependFrame(frame, source);
  const reader = combined.getReader();

  const first = await reader.read();
  assertEquals(first.value, frame);

  const second = await reader.read();
  assertEquals(second.value, sourceBytes);

  const done = await reader.read();
  assertEquals(done.done, true);
});

Deno.test("bodyWithInactivityWatchdog - passes all chunks when stream completes normally", async () => {
  const bytes = [new Uint8Array([1]), new Uint8Array([2]), new Uint8Array([3])];
  let idx = 0;
  const source = new ReadableStream<Uint8Array>({
    pull(controller) {
      if (idx < bytes.length) {
        controller.enqueue(bytes[idx++]);
      } else {
        controller.close();
      }
    },
  });

  const watchdog = bodyWithInactivityWatchdog(source, 5000);
  const reader = watchdog.getReader();
  const collected: Uint8Array[] = [];
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    collected.push(value);
  }
  assertEquals(collected.length, 3);
  assertEquals(collected[0], new Uint8Array([1]));
  assertEquals(collected[1], new Uint8Array([2]));
  assertEquals(collected[2], new Uint8Array([3]));
});

Deno.test(
  "CA12 - bodyWithInactivityWatchdog fires when stream stalls",
  { sanitizeOps: false, sanitizeResources: false },
  async () => {
    let sourceController!: ReadableStreamDefaultController<Uint8Array>;
    const source = new ReadableStream<Uint8Array>({
      start(c) { sourceController = c; },
    });

    const watchdog = bodyWithInactivityWatchdog(source, 60);
    const reader = watchdog.getReader();

    sourceController.enqueue(new Uint8Array([1]));
    const first = await reader.read();
    assertEquals(first.value, new Uint8Array([1]));

    let threw = false;
    reader.read().catch(() => { threw = true; });

    await new Promise<void>((r) => setTimeout(r, 250));

    assertEquals(threw, true);
  },
);

Deno.test(
  "CA12 - headers sent before stream does not disable watchdog",
  { sanitizeOps: false, sanitizeResources: false },
  async () => {
    let sourceController!: ReadableStreamDefaultController<Uint8Array>;
    const source = new ReadableStream<Uint8Array>({
      start(c) { sourceController = c; },
    });

    const watchdog = bodyWithInactivityWatchdog(source, 60);
    const reader = watchdog.getReader();

    sourceController.enqueue(new Uint8Array([42]));
    const firstChunk = await reader.read();
    assertEquals(firstChunk.value, new Uint8Array([42]));

    let timedOut = false;
    reader.read().catch(() => { timedOut = true; });

    await new Promise<void>((r) => setTimeout(r, 250));

    assertEquals(timedOut, true);
  },
);
