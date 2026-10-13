import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { synthesizeInterviewSpeech } from '@/entities/interview';
import { useQuestionSpeech } from './use-question-speech';

vi.mock('@/entities/interview', () => ({ synthesizeInterviewSpeech: vi.fn() }));

const synthesized = { audioBase64: 'YWJj', mimeType: 'audio/wav', provider: 'silero_v5_5', voice: 'eugene' };
const synthesis = vi.mocked(synthesizeInterviewSpeech);
const browserSpeak = vi.fn();
const play = vi.fn<() => Promise<void>>();
const players: FakeAudio[] = [];

class FakeAudio {
  onplay: (() => void) | null = null;
  onended: (() => void) | null = null;
  onerror: (() => void) | null = null;
  pause = vi.fn();
  play = play;
  constructor() {
    players.push(this);
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

beforeEach(() => {
  vi.clearAllMocks();
  players.length = 0;
  synthesis.mockResolvedValue(synthesized);
  play.mockResolvedValue();
  vi.stubGlobal('Audio', FakeAudio);
  vi.stubGlobal('speechSynthesis', { cancel: vi.fn(), getVoices: () => [], speak: browserSpeak });
  vi.stubGlobal('SpeechSynthesisUtterance', class {});
  vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:question');
  vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe('question speech lifecycle', () => {
  it('reports preparation until server audio starts', async () => {
    const pending = deferred<typeof synthesized>();
    synthesis.mockReturnValueOnce(pending.promise);
    const hook = renderHook(() => useQuestionSpeech('interview', 'question', 'Текст', true));
    let request!: Promise<void>;
    act(() => {
      request = hook.result.current.sayQuestion();
    });
    expect(hook.result.current.preparing).toBe(true);
    await act(async () => {
      pending.resolve(synthesized);
      await request;
    });
    expect(hook.result.current.preparing).toBe(true);
    act(() => players[0]?.onplay?.());
    expect(hook.result.current.preparing).toBe(false);
    expect(hook.result.current.speaking).toBe(true);
  });

  it('cancels delayed autoplay when the microphone or stop control takes over', async () => {
    vi.useFakeTimers();
    const hook = renderHook(() => useQuestionSpeech('interview', 'question', 'Текст', true));
    act(() => hook.result.current.stopSpeech());
    await act(() => vi.advanceTimersByTimeAsync(300));
    expect(synthesis).not.toHaveBeenCalled();
    expect(browserSpeak).not.toHaveBeenCalled();
  });

  it('supports browsers without the optional speech API', async () => {
    vi.stubGlobal('speechSynthesis', undefined);
    vi.stubGlobal('SpeechSynthesisUtterance', undefined);
    const hook = renderHook(() => useQuestionSpeech('interview', 'question', 'Текст', false));
    await act(() => hook.result.current.sayQuestion());
    act(() => hook.result.current.stopSpeech());
    hook.unmount();
    expect(synthesis).not.toHaveBeenCalled();
  });

  it.each(['resolve', 'reject'] as const)('ignores synthesis that %ss after stop', async (completion) => {
    const pending = deferred<typeof synthesized>();
    synthesis.mockReturnValueOnce(pending.promise);
    const hook = renderHook(() => useQuestionSpeech('interview', 'question', 'Текст', true));
    let speaking!: Promise<void>;
    act(() => {
      speaking = hook.result.current.sayQuestion();
    });
    act(() => hook.result.current.stopSpeech());
    expect(synthesis.mock.calls[0]?.[2]?.aborted).toBe(true);
    await act(async () => {
      if (completion === 'resolve') pending.resolve(synthesized);
      else pending.reject(new Error('late failure'));
      await speaking;
    });
    expect(players).toHaveLength(0);
    expect(browserSpeak).not.toHaveBeenCalled();
  });

  it.each(['question change', 'unmount'])('ignores a late response after %s', async (change) => {
    const pending = deferred<typeof synthesized>();
    synthesis.mockReturnValueOnce(pending.promise);
    const hook = renderHook(({ id }) => useQuestionSpeech('interview', id, 'Текст', true), {
      initialProps: { id: 'first' },
    });
    let speaking!: Promise<void>;
    act(() => {
      speaking = hook.result.current.sayQuestion();
    });
    if (change === 'unmount') hook.unmount();
    else hook.rerender({ id: 'second' });
    await act(async () => {
      pending.resolve(synthesized);
      await speaking;
    });
    expect(players).toHaveLength(0);
    expect(browserSpeak).not.toHaveBeenCalled();
  });

  it('ignores old play rejection and media callbacks while a new question is playing', async () => {
    const pending = deferred<void>();
    play.mockReturnValueOnce(pending.promise);
    const hook = renderHook(() => useQuestionSpeech('interview', 'question', 'Текст', true));
    let first!: Promise<void>;
    await act(async () => {
      first = hook.result.current.sayQuestion();
      await Promise.resolve();
    });
    const stalePlay = players[0]?.onplay;
    const staleEnd = players[0]?.onended;
    const staleError = players[0]?.onerror;
    await act(() => hook.result.current.sayQuestion());
    act(() => players[1]?.onplay?.());
    await act(async () => {
      stalePlay?.();
      staleEnd?.();
      staleError?.();
      pending.reject(new Error('old play rejected'));
      await first;
    });
    expect(hook.result.current.speaking).toBe(true);
    expect(players[0]?.pause).toHaveBeenCalledOnce();
    expect(players[0]?.onerror).toBeNull();
    expect(browserSpeak).not.toHaveBeenCalled();
    expect(URL.revokeObjectURL).toHaveBeenCalledWith('blob:question');
  });

  it('falls back once for a current audio error even if play also rejects', async () => {
    const pending = deferred<void>();
    play.mockReturnValueOnce(pending.promise);
    const hook = renderHook(() => useQuestionSpeech('interview', 'question', 'Текст', true));
    let speaking!: Promise<void>;
    await act(async () => {
      speaking = hook.result.current.sayQuestion();
      await Promise.resolve();
    });
    await act(async () => {
      players[0]?.onerror?.();
      pending.reject(new Error('play rejected'));
      await speaking;
    });
    expect(browserSpeak).toHaveBeenCalledOnce();
  });
});
