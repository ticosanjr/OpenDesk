import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/api", () => ({ apiPostBlob: vi.fn() }));

import { apiPostBlob } from "@/lib/api";
import { useVoiceOutput } from "./use-voice-output";

class FakeAudio {
  static instances: FakeAudio[] = [];
  src = "";
  onended: (() => void) | null = null;
  onerror: (() => void) | null = null;
  play = vi.fn(async () => undefined);
  pause = vi.fn();
  constructor(src: string) {
    this.src = src;
    FakeAudio.instances.push(this);
  }
}

beforeEach(() => {
  vi.clearAllMocks();
  FakeAudio.instances = [];
  vi.stubGlobal("Audio", FakeAudio);
  vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:tts");
  vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => undefined);
  (apiPostBlob as ReturnType<typeof vi.fn>).mockResolvedValue(new Blob(["wav"]));
});

describe("useVoiceOutput", () => {
  it("plays the returned blob and revokes it when audio ends", async () => {
    const { result } = renderHook(() => useVoiceOutput());
    await act(async () => {
      await result.current.play("olá", "ws-1");
    });
    expect(apiPostBlob).toHaveBeenCalledWith("/api/voice/tts", {
      text: "olá",
      workspace_id: "ws-1",
    });
    expect(URL.createObjectURL).toHaveBeenCalledTimes(1);
    expect(result.current.speaking).toBe(true);
    const audio = FakeAudio.instances[0];
    expect(audio.play).toHaveBeenCalled();
    act(() => audio.onended?.());
    expect(result.current.speaking).toBe(false);
    expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:tts");
  });

  it("ignores empty text and reports audio element errors", async () => {
    const { result } = renderHook(() => useVoiceOutput());
    await act(async () => {
      await result.current.play("   ");
    });
    expect(apiPostBlob).not.toHaveBeenCalled();
    await act(async () => {
      await result.current.play("olá");
    });
    const audio = FakeAudio.instances[0];
    act(() => audio.onerror?.());
    expect(result.current.error).toBe("O áudio não pôde ser reproduzido.");
    expect(result.current.speaking).toBe(false);
  });

  it("stops playback and reports backend errors", async () => {
    const { result } = renderHook(() => useVoiceOutput());
    await act(async () => {
      await result.current.play("olá");
    });
    act(() => result.current.stop());
    expect(FakeAudio.instances[0].pause).toHaveBeenCalled();
    expect(result.current.speaking).toBe(false);

    (apiPostBlob as ReturnType<typeof vi.fn>).mockRejectedValue(
      new Error("sem TTS"),
    );
    await act(async () => {
      await result.current.play("erro");
    });
    await waitFor(() => expect(result.current.error).toBe("sem TTS"));
    expect(result.current.speaking).toBe(false);

    (apiPostBlob as ReturnType<typeof vi.fn>).mockRejectedValue("texto");
    await act(async () => {
      await result.current.play("erro");
    });
    await waitFor(() => expect(result.current.error).toBe("texto"));
  });
});
