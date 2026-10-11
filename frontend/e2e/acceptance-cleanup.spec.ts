import { expect, test } from '@playwright/test';
import type { components } from '../src/shared/api/generated/schema';
import { acceptanceActors } from './public/actors';
import { resetMock, signIn } from './support';

test('failed acceptance removes its publication and needs while preserving other actors', async ({ browser, page }) => {
  await resetMock('baseline');
  const actors = acceptanceActors(browser);
  const candidateEmail = `fsp-ci-candidate-${Date.now()}@example.com`;
  const employerEmail = `fsp-ci-employer-${Date.now()}@example.com`;
  const candidate = await actors.newPage();
  const employer = await actors.newPage();
  const run = async () => {
    try {
      await signIn(candidate, candidateEmail, 'candidate');
      actors.authenticated(candidate, candidateEmail, 'candidate');
      expect(
        (
          await candidate.request.patch('/api/v1/me/profile', {
            data: { consents: { processing: true, publication: true } },
          })
        ).ok(),
      ).toBe(true);
      await signIn(employer, employerEmail, 'employer');
      actors.authenticated(employer, employerEmail, 'employer');
      const need: components['schemas']['NeedInput'] = {
        title: 'Cleanup regression',
        specialization: 'backend',
        grade: 'junior',
        workMode: 'remote',
        salary: { from: 80000, to: 120000, currency: 'RUB', period: 'month', taxBasis: 'gross' },
        description: 'Temporary acceptance need',
        requirements: [
          {
            skillId: 'rust',
            minimumGrade: 'junior',
            criticality: 'required',
            evidencePolicy: {
              acceptAssessments: true,
              acceptGithub: false,
              acceptFsp: false,
              competitionSelection: 'system',
              competitionIds: [],
            },
          },
        ],
      };
      expect((await employer.request.post('/api/v1/needs', { data: need })).status()).toBe(201);
      throw new Error('Injected failure after persistent writes');
    } finally {
      await actors.dispose();
    }
  };
  await expect(run()).rejects.toThrow('Injected failure after persistent writes');
  expect(candidate.isClosed()).toBe(true);
  expect(employer.isClosed()).toBe(true);

  await signIn(page, candidateEmail, 'candidate');
  const profile = await (await page.request.get('/api/v1/me/profile')).json();
  expect(profile.consents).toEqual({ processing: true, publication: false });
  await page.request.post('/api/v1/auth/logout');
  await signIn(page, employerEmail, 'employer');
  expect((await (await page.request.get('/api/v1/needs')).json()).items).toEqual([]);
  // This account has no access to the baseline company's need; teardown must not touch it.
  await page.request.post('/api/v1/auth/logout');
  await signIn(page, 'hr@sever-tech.example', 'employer');
  expect((await page.request.get('/api/v1/needs/need-001')).status()).toBe(200);
});

test('cleanup reports an identity mismatch and still closes every owned context', async ({ browser, page }) => {
  await resetMock('baseline');
  const actors = acceptanceActors(browser);
  const candidate = await actors.newPage();
  const goodCandidate = await actors.newPage();
  const goodEmail = `fsp-ci-good-${Date.now()}@example.com`;
  await signIn(candidate, `fsp-ci-failed-${Date.now()}@example.com`, 'candidate');
  actors.authenticated(candidate, 'fsp-ci-wrong-identity@example.com', 'candidate');
  await signIn(goodCandidate, goodEmail, 'candidate');
  actors.authenticated(goodCandidate, goodEmail, 'candidate');
  expect(
    (
      await goodCandidate.request.patch('/api/v1/me/profile', {
        data: { consents: { processing: true, publication: true } },
      })
    ).ok(),
  ).toBe(true);
  await expect(actors.dispose()).rejects.toThrow('Acceptance cleanup failed');
  expect(candidate.isClosed()).toBe(true);
  expect(goodCandidate.isClosed()).toBe(true);
  await signIn(page, goodEmail, 'candidate');
  expect((await (await page.request.get('/api/v1/me/profile')).json()).consents.publication).toBe(false);
});
