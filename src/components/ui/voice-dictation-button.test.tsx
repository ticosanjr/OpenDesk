import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/use-voice-input", () => ({ useVoiceInput: vi.fn() }));

import { useVoiceInput } from "@/lib/use-voice-input";
import { VoiceDictationButton } from "./voice-dictation-button";

const start = vi.fn();
const stop = vi.fn();

function mockVoice(overrides: Record<string, unknown> = {}) {
  vi.mocked(useVoiceInput).mockReturnValue({
    recording: false,
    transcribing: false,
    seconds: 0,
    error: null,
    start,
    stop,
    cancel: vi.fn(),
    clearError: vi.fn(),
    ...overrides,
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  mockVoice();
});

describe("VoiceDictationButton", () => {
  it("starts dictation and exposes an accessible label", () => {
    render(<VoiceDictationButton onTranscript={vi.fn()} label="Ditar tarefa" />);
    const button = screen.getByRole("button", { name: "Ditar tarefa" });
    expect(button).toHaveAttribute("aria-pressed", "false");
    fireEvent.click(button);
    expect(start).toHaveBeenCalledTimes(1);
  });

  it("stops while recording and shows the elapsed time", () => {
    mockVoice({ recording: true, seconds: 4 });
    render(<VoiceDictationButton onTranscript={vi.fn()} />);
    const button = screen.getByRole("button", { name: "Parar gravação" });
    expect(button).toHaveAttribute("aria-pressed", "true");
    expect(button).toHaveAttribute("title", "Parar gravação (4s)");
    fireEvent.click(button);
    expect(stop).toHaveBeenCalledTimes(1);
  });

  it("disables the button while transcribing", () => {
    mockVoice({ transcribing: true });
    render(<VoiceDictationButton onTranscript={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Ditar por voz" })).toBeDisabled();
  });
});
