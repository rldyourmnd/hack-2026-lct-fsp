import { queryOptions } from '@tanstack/react-query';
import { api, type components, requireData, type Scope } from '@/shared/api';

export type Interview = components['schemas']['Interview'];
export type InterviewNode = components['schemas']['InterviewNode'];
export type InterviewAnswerInput = components['schemas']['InterviewAnswerInput'];
export type SpeechAudio = components['schemas']['SpeechAudio'];

export const interviewsKey = (scope: Scope) => [...scope, 'interviews'] as const;
export const interviewKey = (scope: Scope, id: string) => [...scope, 'interviews', id] as const;

export const interviewsQuery = (scope: Scope) =>
  queryOptions({
    queryKey: interviewsKey(scope),
    queryFn: async ({ signal }) => requireData(await api.GET('/api/v1/me/interviews', { signal })),
  });

export const interviewQuery = (scope: Scope, id: string) =>
  queryOptions({
    queryKey: interviewKey(scope, id),
    queryFn: async ({ signal }) =>
      requireData(
        await api.GET('/api/v1/me/interviews/{interviewId}', {
          params: { path: { interviewId: id } },
          signal: signal ?? null,
        }),
      ),
  });

export async function startInterview() {
  return requireData(
    await api.POST('/api/v1/me/interviews', {
      body: { blueprintId: 'backend-middle-v1', maxTurns: 12 },
    }),
  );
}

export async function answerInterview(id: string, input: InterviewAnswerInput) {
  return requireData(
    await api.POST('/api/v1/me/interviews/{interviewId}/answers', {
      params: { path: { interviewId: id } },
      body: input,
    }),
  );
}

export async function synthesizeInterviewSpeech(id: string, questionId: string, signal?: AbortSignal) {
  return requireData(
    await api.POST('/api/v1/me/interviews/{interviewId}/speech', {
      params: { path: { interviewId: id } },
      body: { questionId },
      signal: signal ?? null,
    }),
  );
}

export async function transcribeInterviewSpeech(
  id: string,
  questionId: string,
  audioBase64: string,
  mimeType: 'audio/webm' | 'audio/mp4' | 'audio/ogg',
) {
  return requireData(
    await api.POST('/api/v1/me/interviews/{interviewId}/transcription', {
      params: { path: { interviewId: id } },
      body: { questionId, audioBase64, mimeType },
    }),
  );
}
