// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { speechErrorMessage, submittedResponseMode } from './speech';

describe('graph interview response modes', () => {
  it('counts voice only after speech produced a transcript', () => {
    expect(submittedResponseMode('voice', false)).toBe('text');
    expect(submittedResponseMode('voice', true)).toBe('voice');
    expect(submittedResponseMode('text', true)).toBe('text');
  });

  it('explains permission, silence, microphone and network failures separately', () => {
    expect(speechErrorMessage('not-allowed')).toContain('доступ к микрофону');
    expect(speechErrorMessage('no-speech')).toContain('не была услышана');
    expect(speechErrorMessage('audio-capture')).toContain('не видит микрофон');
    expect(speechErrorMessage('network')).toContain('Сервис распознавания');
  });
});
