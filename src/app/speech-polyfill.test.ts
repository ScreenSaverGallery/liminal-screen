// Tests for the speechSynthesis polyfill injected into saver/preview windows.
// The shim lives in src-tauri (embedded via include_str!); here we evaluate it
// against fake `window` objects to verify its runtime behavior without a
// webview.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it, vi } from "vitest";

const polyfillSource = readFileSync(
  fileURLToPath(new URL("../../src-tauri/src/speech_polyfill.js", import.meta.url)),
  "utf-8",
);

interface FakeWindow {
  [key: string]: unknown;
  console?: { warn: (...args: unknown[]) => void; error: (...args: unknown[]) => void };
}

function runPolyfill(win: FakeWindow): FakeWindow {
  new Function("window", polyfillSource)(win);
  return win;
}

const flushQueue = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("speech polyfill", () => {
  it("steps aside when a native speechSynthesis exists", () => {
    const native = { native: true };
    const win = runPolyfill({ speechSynthesis: native });
    expect(win.speechSynthesis).toBe(native);
    expect(win.SpeechSynthesisUtterance).toBeUndefined();
  });

  it("warns and installs nothing without an IPC bridge", () => {
    const warn = vi.fn();
    const win = runPolyfill({ console: { warn, error: vi.fn() } });
    expect(win.speechSynthesis).toBeUndefined();
    expect(win.SpeechSynthesisUtterance).toBeUndefined();
    expect(warn).toHaveBeenCalledOnce();
  });

  it("installs the shim and forwards speak() to the speak_text command", async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    const win = runPolyfill({ __TAURI_INTERNALS__: { invoke } });

    expect(win.speechSynthesis).toBeDefined();
    const Utterance = win.SpeechSynthesisUtterance as new (text?: string) => any;
    const synth = win.speechSynthesis as any;

    const utterance = new Utterance("hello");
    utterance.rate = 2;
    utterance.lang = "en-US";
    const events: string[] = [];
    utterance.onstart = () => events.push("start");
    utterance.onend = () => events.push("end");
    utterance.onerror = () => events.push("error");

    synth.speak(utterance);
    expect(synth.speaking).toBe(true);
    await flushQueue();

    expect(invoke).toHaveBeenCalledWith("speak_text", {
      text: "hello",
      rate: 2,
      pitch: 1,
      volume: 1,
      lang: "en-US",
    });
    expect(events).toEqual(["start", "end"]);
    expect(synth.speaking).toBe(false);
  });

  it("fires error (not end) when the command rejects", async () => {
    const invoke = vi.fn().mockRejectedValue("spd-say missing");
    const win = runPolyfill({ __TAURI_INTERNALS__: { invoke } });
    const Utterance = win.SpeechSynthesisUtterance as new (text?: string) => any;
    const synth = win.speechSynthesis as any;

    const utterance = new Utterance("hello");
    const events: Array<{ type: string; error?: string }> = [];
    utterance.onend = (e: any) => events.push({ type: e.type });
    utterance.onerror = (e: any) => events.push({ type: e.type, error: e.error });

    synth.speak(utterance);
    await flushQueue();

    expect(events).toEqual([{ type: "error", error: "spd-say missing" }]);
    expect(synth.speaking).toBe(false);
  });

  it("queues utterances and drains the queue on cancel()", async () => {
    let resolveFirst!: () => void;
    const invoke = vi.fn().mockImplementation((cmd: string) => {
      if (cmd === "speak_text") {
        return new Promise<void>((resolve) => {
          resolveFirst = resolve;
        });
      }
      return Promise.resolve();
    });
    const win = runPolyfill({ __TAURI_INTERNALS__: { invoke } });
    const Utterance = win.SpeechSynthesisUtterance as new (text?: string) => any;
    const synth = win.speechSynthesis as any;

    const first = new Utterance("first");
    const second = new Utterance("second");
    const secondEvents: string[] = [];
    second.onstart = () => secondEvents.push("start");
    second.onend = () => secondEvents.push("end");

    synth.speak(first);
    synth.speak(second);
    await flushQueue();
    expect(synth.pending).toBe(true);

    // cancel while `first` is mid-utterance: `second` must never speak
    synth.cancel();
    expect(invoke).toHaveBeenCalledWith("cancel_speech");
    resolveFirst();
    await flushQueue();

    expect(secondEvents).toEqual(["end"]);
    expect(
      invoke.mock.calls.filter((call) => call[0] === "speak_text"),
    ).toHaveLength(1);
    expect(synth.speaking).toBe(false);
  });

  it("provides the inert parts of the native surface", () => {
    const win = runPolyfill({ __TAURI_INTERNALS__: { invoke: vi.fn() } });
    const synth = win.speechSynthesis as any;
    expect(synth.getVoices()).toEqual([]);
    expect(synth.paused).toBe(false);
    expect(() => {
      synth.pause();
      synth.resume();
      synth.addEventListener("voiceschanged", () => {});
      synth.removeEventListener("voiceschanged", () => {});
    }).not.toThrow();
  });
});
