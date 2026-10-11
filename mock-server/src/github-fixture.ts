import { readFileSync } from 'node:fs';
import type { components } from './generated/schema.d.ts';

type S = components['schemas'];

/**
 * Behavior fixture for the development API; the Rust backend asks the insights service.
 *
 * The report and its employer projection are the golden files the Rust analysis writes to
 * `contracts/fixtures/` from a synthetic account, so the mock never drifts from the real
 * serialization. The mock suite validates both against the contract schemas.
 */
const fixture = (name: string): unknown =>
  JSON.parse(readFileSync(new URL(`../../contracts/fixtures/${name}`, import.meta.url), 'utf8'));
const REPORT = fixture('github-report.json') as S['GitHubReport'];
const SIGNAL = fixture('github-signal.json') as S['GitHubSignal'];

/** Normalizes `login`, `@login` or a profile URL the way the Rust domain does; null when invalid. */
export function parseGitHubLogin(input: string): string | null {
  let value = input
    .trim()
    .replace(/^https?:\/\//, '')
    .replace(/^www\./, '');
  if (value.startsWith('github.com/')) {
    const segments = value.slice('github.com/'.length).split(/[?#]/)[0]?.split('/').filter(Boolean) ?? [];
    if (segments.length !== 1) return null;
    value = segments[0] ?? '';
  } else if (value.includes('/') || value.includes('.')) {
    return null;
  } else if (value.startsWith('@')) {
    value = value.slice(1);
  }
  return /^[A-Za-z0-9](?:[A-Za-z0-9]|-(?=[A-Za-z0-9])){0,38}$/.test(value) ? value : null;
}

/** The synthetic report under `login`; links point to a fixture host, never to real accounts. */
export function githubFixtureReport(login: string, clock: string): S['GitHubReport'] {
  const text = JSON.stringify(REPORT)
    .replaceAll(`"${REPORT.login}/`, `"${login}/`)
    .replaceAll(`github.com/${REPORT.login}/`, `github.com/${login}/`)
    .replaceAll('https://github.com/', 'https://example.com/fixtures/github/');
  return {
    ...(JSON.parse(text) as S['GitHubReport']),
    login,
    profileUrl: `https://example.com/fixtures/github/${login}`,
    generatedAt: clock,
  };
}

/** The anonymous employer projection: identical for every fixture report but the verification. */
export function githubSignal(report: S['GitHubReport'], ownershipVerified: boolean): S['GitHubSignal'] {
  return { ...structuredClone(SIGNAL), generatedAt: report.generatedAt.slice(0, 10), ownershipVerified };
}

/** The first stage a fresh analysis reports, as the insights service does. */
export const GITHUB_ANALYSIS_STARTED: NonNullable<S['GitHubSource']['analysis']> = {
  state: 'running',
  stage: 'Поиск коммитов по периодам',
  progress: 6,
};

/** The GitHub part of a source: a deterministic code, and the report when analyzed. */
export function githubSource(
  candidateId: string,
  report: S['GitHubReport'] | null,
  verifiedAt: string | null,
): S['GitHubSource'] {
  return {
    ownership: {
      verified: verifiedAt !== null,
      method: verifiedAt === null ? null : 'profile_code',
      verifiedAt,
      code: `fsp-demo${candidateId
        .replace(/[^a-z0-9]/gi, '')
        .slice(-6)
        .toLowerCase()}`,
    },
    report,
    employerPreview: report && githubSignal(report, verifiedAt !== null),
    analysis: null,
  };
}
