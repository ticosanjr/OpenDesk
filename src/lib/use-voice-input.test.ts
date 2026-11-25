import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/api", () => ({ apiPost: vi.fn() }));

import { apiPost } from "@/lib/api";
import {
  appendTranscript,
  insertAtCursor,
  useVoiceInput,
} from "./use-voice-input";

class FakeRecorder {
  static instances: FakeRecorder[] = [];
  static isTypeSupported = () => true;
  mimeType = "audio/webm";
  state = "inactive";
  ondataavailable: ((event: { data: Blob }) => void) | null = null;
  onstop: (() => void) | null = null;
  constructor(_stream: unknown) {
    FakeRecorder.instances.push(this);
  }
  start() {
    this.state = "recording";
  }
  stop() {
    this.state = "inactive";
    this.ondataavailable?.({ data: new Blob(["audio"]) });
    this.onstop?.();
  }
}

const stopTrack = vi.fn();
const stream = {
  getTracks: () => [{ stop: stopTrack }],
};

beforeEach(() => {
  vi.clearAllMocks();
  FakeRecorder.instances = [];
  vi.stubGlobal("MediaRecorder", FakeRecorder);
  Object.defineProperty(navigator, "mediaDevices", {
    configurable: true,
    value: { getUserMedia: vi.fn(async () => stream) },
  });
  (apiPost as ReturnType<typeof vi.fn>).mockResolvedValue({
    text: "texto falado",
    provider: "whisper",
  });
});

describe("voice transcript placement", () => {
  it("appends without duplicating spaces", () => {
    expect(appendTranscript("", "  olá ")).toBe("olá");
    expect(appendTranscript("texto", "continuação")).toBe("texto continuação");
    expect(appendTranscript("texto ", " continuação")).toBe("texto continuação");
  });

  it("inserts at the caret when a field is available", () => {
    const element = {
      selectionStart: 2,
      selectionEnd: 4,
    } as unknown as HTMLTextAreaElement;
    expect(insertAtCursor(element, "abcdef", "XY")).toBe("abXYef");
    expect(insertAtCursor(null, "abc", "def")).toBe("abc def");
  });
});

describe("useVoiceInput", () => {
  it("records, stops tracks and sends the transcript", async () => {
    const onTranscript = vi.fn();
    const { result } = renderHook(() => useVoiceInput({ onTranscript }));
    await act(async () => {
      result.current.start();
    });
    await waitFor(() => expect(result.current.recording).toBe(true));
    act(() => result.current.stop());
    await waitFor(() => expect(onTranscript).toHaveBeenCalledWith({
      text: "texto falado",
      provider: "whisper",
    }));
    expect(stopTrack).toHaveBeenCalled();
    expect(apiPost).toHaveBeenCalledWith("/api/voice/stt", {
      audio_base64: expect.any(String),
      mime: "audio/webm",
    });
    expect(result.current.transcribing).toBe(false);
  });

  it("can stop when nothing is recording", () => {
    const { result } = renderHook(() => useVoiceInput({ onTranscript: vi.fn() }));
    act(() => result.current.stop());
    expect(result.current.recording).toBe(false);
  });

  it("reports microphone permission errors", async () => {
    const onError = vi.fn();
    Object.defineProperty(navigator, "mediaDevices", {
      configurable: true,
      value: {
        getUserMedia: vi.fn(async () => {
          throw new DOMException("denied", "NotAllowedError");
        }),
      },
    });
    const { result } = renderHook(() => useVoiceInput({ onTranscript: vi.fn(), onError }));
    await act(async () => {
      result.current.start();
    });
    await waitFor(() => expect(result.current.error).toContain("Permissão"));
    expect(onError).toHaveBeenCalled();
  });

  it("cancels without transcribing and releases the stream", async () => {
    const onTranscript = vi.fn();
    const { result } = renderHook(() => useVoiceInput({ onTranscript }));
    await act(async () => {
      result.current.start();
    });
    await waitFor(() => expect(result.current.recording).toBe(true));
    act(() => result.current.cancel());
    await waitFor(() => expect(result.current.recording).toBe(false));
    expect(onTranscript).not.toHaveBeenCalled();
    expect(stopTrack).toHaveBeenCalled();
  });
});

describe("useVoiceInput failures and timer", () => {
  it("reports transcription failures and clears the error", async () => {
    (apiPost as ReturnType<typeof vi.fn>).mockRejectedValue(new Error("STT fora"));
    const { result } = renderHook(() => useVoiceInput({ onTranscript: vi.fn() }));
    await act(async () => {
      result.current.start();
    });
    await waitFor(() => expect(result.current.recording).toBe(true));
    act(() => result.current.stop());
    await waitFor(() => expect(result.current.error).toBe("STT fora"));
    act(() => result.current.clearError());
    expect(result.current.error).toBeNull();
  });

  it("handles non-Error microphone failures and elapsed time", async () => {
    Object.defineProperty(navigator, "mediaDevices", {
      configurable: true,
      value: { getUserMedia: vi.fn(async () => Promise.reject("boom")) },
    });
    const first = renderHook(() => useVoiceInput({ onTranscript: vi.fn() }));
    await act(async () => {
      first.result.current.start();
    });
    await waitFor(() =>
      expect(first.result.current.error).toBe("Microfone indisponível."),
    );

    vi.useFakeTimers();
    Object.defineProperty(navigator, "mediaDevices", {
      configurable: true,
      value: { getUserMedia: vi.fn(async () => stream) },
    });
    const second = renderHook(() => useVoiceInput({ onTranscript: vi.fn() }));
    await act(async () => {
      second.result.current.start();
      await Promise.resolve();
    });
    act(() => vi.advanceTimersByTime(2000));
    expect(second.result.current.seconds).toBe(2);
    act(() => second.result.current.stop());
    vi.useRealTimers();
  });
});
