import { inflateSync } from 'node:zlib';
import { expect, test } from '@playwright/test';
import { ALEXEY, PRIVATE_MARKERS, resetMock, signIn } from './support';

test('RUM and actual compressed replay payloads exclude private canaries', async ({ page }) => {
  test.skip(process.env.FSP_RUM_TEST !== '1', 'Run with FSP_RUM_TEST=1 VITE_OBSERVABILITY=1');
  test.setTimeout(90_000);
  await resetMock();
  const rum: string[] = [];
  const replay: string[] = [];
  const external: string[] = [];
  page.on('request', (request) => {
    if (/^https?:/.test(request.url()) && new URL(request.url()).hostname !== '127.0.0.1') external.push(request.url());
  });
  await page.route('**/observe/*', async (route) => {
    const request = route.request();
    expect(new URL(request.url()).search).toBe('');
    const body = request.postDataBuffer();
    if (body && request.url().endsWith('/replay')) {
      const form = await new Response(new Uint8Array(body), {
        headers: { 'content-type': request.headers()['content-type'] ?? '' },
      }).formData();
      const segment = form.get('segment');
      expect(segment).toBeInstanceOf(Blob);
      if (segment instanceof Blob) replay.push(inflateSync(Buffer.from(await segment.arrayBuffer())).toString());
    } else if (body) rum.push(body.toString());
    if (process.env.FSP_RUM_UPSTREAM && process.env.FSP_RUM_TOKEN && body) {
      const endpoint = request.url().endsWith('/replay') ? 'replay' : 'rum';
      const response = await fetch(`${process.env.FSP_RUM_UPSTREAM}/rum/v1/default/${endpoint}`, {
        method: 'POST',
        headers: { 'content-type': request.headers()['content-type'] ?? '', 'oo-api-key': process.env.FSP_RUM_TOKEN },
        body: new Uint8Array(body),
      });
      expect(response.status).toBe(202);
      await route.fulfill({ status: response.status, body: await response.text() });
    } else await route.fulfill({ status: 202 });
  });
  await signIn(page, ALEXEY, 'candidate');
  await page.goto('/candidate/vacancies/need-002');
  await expect(page.getByLabel('Сопроводительное письмо')).toBeVisible();
  const canary = 'PRIVATE-CANARY-REPLAY-983';
  await page.getByLabel('Сопроводительное письмо').fill(canary);
  await page.evaluate((secret) => {
    const box = document.createElement('div');
    box.textContent = secret;
    box.setAttribute('data-private', secret);
    box.setAttribute('aria-label', secret);
    const link = document.createElement('a');
    link.href = `/candidate/profile?secret=${secret}`;
    link.textContent = secret;
    box.append(link);
    document.body.append(box);
    void fetch(`/api/v1/me/profile?secret=${secret}`);
    setTimeout(() => {
      throw new Error(secret);
    }, 0);
  }, canary);
  await expect.poll(() => replay.length, { timeout: 20_000 }).toBeGreaterThan(0);
  await expect.poll(() => rum.length, { timeout: 45_000 }).toBeGreaterThan(0);
  // A new document with secret query parameters must never start replay recording.
  await page.goto(`/candidate/profile?secret=${canary}`);
  await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
  await page.goto('/candidate/vacancies/need-002');
  await expect.poll(() => replay.length, { timeout: 20_000 }).toBeGreaterThan(1);
  const wire = [...rum, ...replay].join('\n');
  for (const value of [...PRIVATE_MARKERS, canary, ALEXEY]) expect(wire).not.toContain(value);
  expect(wire).toContain('fsp-career');
  expect(replay.some((part) => part.includes('"records"'))).toBe(true);
  expect(external).toEqual([]);
});
