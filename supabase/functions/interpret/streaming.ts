export const BODY_INACTIVITY_MS = 30_000;

export const METADATA_MAGIC = new Uint8Array([0x56, 0x4c, 0x42, 0x58]);

export function buildMetadataFrame(sourceText: string): Uint8Array {
  const capped = [...sourceText].slice(0, 300).join("");
  const json = JSON.stringify({ sourceText: capped });
  const jsonBytes = new TextEncoder().encode(json);

  const frame = new Uint8Array(4 + 4 + jsonBytes.byteLength);
  frame.set(METADATA_MAGIC, 0);
  new DataView(frame.buffer).setUint32(4, jsonBytes.byteLength, false);
  frame.set(jsonBytes, 8);
  return frame;
}

export function bodyWithInactivityWatchdog(
  source: ReadableStream<Uint8Array>,
  inactivityMs: number,
): ReadableStream<Uint8Array> {
  let timeoutHandle: ReturnType<typeof setTimeout> | undefined;
  let streamController: ReadableStreamDefaultController<Uint8Array>;
  let timedOut = false;

  const output = new ReadableStream<Uint8Array>({
    start(controller) {
      streamController = controller;
    },
    cancel() {
      clearTimeout(timeoutHandle);
    },
  });

  const pump = async () => {
    const reader = source.getReader();

    const resetTimer = () => {
      clearTimeout(timeoutHandle);
      timeoutHandle = setTimeout(() => {
        timedOut = true;
        reader.cancel().catch(() => {});
      }, inactivityMs);
    };

    resetTimer();

    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) {
          clearTimeout(timeoutHandle);
          if (timedOut) {
            try {
              streamController.error(new Error("STREAM_INACTIVITY_TIMEOUT"));
            } catch {
            }
          } else {
            try {
              streamController.close();
            } catch {
            }
          }
          break;
        }
        resetTimer();
        try {
          streamController.enqueue(value);
        } catch {
          break;
        }
      }
    } catch (err) {
      clearTimeout(timeoutHandle);
      try {
        streamController.error(err);
      } catch {
      }
    } finally {
      reader.releaseLock();
    }
  };

  pump();
  return output;
}

export function prependFrame(
  frame: Uint8Array,
  source: ReadableStream<Uint8Array>,
): ReadableStream<Uint8Array> {
  let frameSent = false;
  let reader: ReadableStreamDefaultReader<Uint8Array> | undefined;

  return new ReadableStream<Uint8Array>({
    async pull(controller) {
      if (!frameSent) {
        frameSent = true;
        controller.enqueue(frame);
        return;
      }
      if (!reader) {
        reader = source.getReader();
      }
      const { done, value } = await reader.read();
      if (done) {
        controller.close();
        return;
      }
      controller.enqueue(value);
    },
    cancel() {
      reader?.cancel().catch(() => {});
    },
  });
}
