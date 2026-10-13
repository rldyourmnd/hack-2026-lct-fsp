import { describe, expect, it } from 'vitest';
import { canReplay, safeUrl } from './privacy';

describe('diagnostic URL privacy', () => {
  it('removes identities, queries, fragments and external origins from RUM URLs', () => {
    const origin = window.location.origin;
    expect(safeUrl(`${origin}/api/v1/candidates/private-email?token=canary#secret`)).toBe(
      `${origin}/api/v1/candidates/:id`,
    );
    expect(safeUrl('https://private.example/alexey?token=canary')).toBe(`${origin}/external`);
    expect(safeUrl('http://[')).toBe(`${origin}/unknown`);
  });

  it('accepts actual generated candidate and need IDs, but no arbitrary labels', () => {
    expect(canReplay('/employer/candidates/candidate-abc123def456?needId=need-0123456789abcdef0123456789abcdef')).toBe(
      true,
    );
    expect(canReplay('/employer/candidates/candidate024?needId=need-b-001')).toBe(true);
    expect(canReplay('/candidate/vacancies/need-002')).toBe(true);
    for (const url of [
      '/candidate/profile?email=canary',
      '/candidate/profile#canary',
      '/employer/candidates/private-name',
      '/candidate/vacancies/need-private-name',
      '/candidate/profile?needId=need-private-name',
      '/candidate/settings',
      '/candidate/assessments/test',
      '/candidate/interviews/test',
      '/login',
    ])
      expect(canReplay(url)).toBe(false);
  });
});
