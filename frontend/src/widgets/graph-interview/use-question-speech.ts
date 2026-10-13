import { useCallback, useEffect, useRef, useState } from 'react';
import { synthesizeInterviewSpeech } from '@/entities/interview';

export function useQuestionSpeech(
  interviewId: string,
  questionId: string | undefined,
  text: string | undefined,
  neuralTts: boolean,
) {
  const generation = useRef(0);
  const startupTimer = useRef<number | null>(null);
  const request = useRef<AbortController | null>(null);
  const audio = useRef<HTMLAudioElement | null>(null);
  const audioUrl = useRef<string | null>(null);
  const utterance = useRef<SpeechSynthesisUtterance | null>(null);
  const [speaking, setSpeaking] = useState(false);
  const [preparing, setPreparing] = useState(false);

  const stopSpeech = useCallback(() => {
    generation.current += 1;
    if (startupTimer.current !== null) window.clearTimeout(startupTimer.current);
    startupTimer.current = null;
    request.current?.abort();
    request.current = null;
    if (utterance.current) {
      utterance.current.onstart = null;
      utterance.current.onend = null;
      utterance.current.onerror = null;
      utterance.current = null;
    }
    window.speechSynthesis?.cancel();
    if (audio.current) {
      audio.current.onplay = null;
      audio.current.onended = null;
      audio.current.onerror = null;
      audio.current.pause();
      audio.current = null;
    }
    if (audioUrl.current) URL.revokeObjectURL(audioUrl.current);
    audioUrl.current = null;
    setSpeaking(false);
    setPreparing(false);
  }, []);

  const sayWithBrowser = useCallback(
    (message: string) => {
      const synthesis = window.speechSynthesis;
      if (!synthesis || typeof window.SpeechSynthesisUtterance !== 'function') return;
      const current = generation.current;
      const speech = new SpeechSynthesisUtterance(message);
      utterance.current = speech;
      speech.lang = 'ru-RU';
      speech.voice =
        synthesis
          .getVoices()
          .filter((voice) => voice.lang.toLowerCase().startsWith('ru'))
          .sort((left, right) => Number(right.localService) - Number(left.localService))[0] ?? null;
      speech.onstart = () => {
        if (generation.current === current) setSpeaking(true);
      };
      speech.onend = speech.onerror = () => {
        if (generation.current === current) stopSpeech();
      };
      synthesis.speak(speech);
    },
    [stopSpeech],
  );

  const sayQuestion = useCallback(async () => {
    if (!text || !questionId) return;
    stopSpeech();
    if (!neuralTts) {
      sayWithBrowser(text);
      return;
    }
    const current = generation.current;
    const controller = new AbortController();
    request.current = controller;
    setPreparing(true);
    const fallback = () => {
      if (generation.current !== current) return;
      stopSpeech();
      sayWithBrowser(text);
    };
    try {
      const synthesized = await synthesizeInterviewSpeech(interviewId, questionId, controller.signal);
      if (generation.current !== current) return;
      const bytes = Uint8Array.from(atob(synthesized.audioBase64), (character) => character.charCodeAt(0));
      audioUrl.current = URL.createObjectURL(new Blob([bytes], { type: synthesized.mimeType }));
      const player = new Audio(audioUrl.current);
      audio.current = player;
      player.onplay = () => {
        if (generation.current === current) {
          setPreparing(false);
          setSpeaking(true);
        }
      };
      player.onended = () => {
        if (generation.current === current) stopSpeech();
      };
      player.onerror = fallback;
      await player.play();
    } catch {
      fallback();
    }
  }, [interviewId, questionId, text, neuralTts, sayWithBrowser, stopSpeech]);

  // Question changes and unmount invalidate network, playback and browser callbacks together.
  useEffect(() => {
    if (!questionId) return undefined;
    startupTimer.current = window.setTimeout(() => {
      startupTimer.current = null;
      void sayQuestion();
    }, 250);
    return stopSpeech;
  }, [questionId, stopSpeech, sayQuestion]);

  return { preparing, speaking, sayQuestion, stopSpeech };
}
