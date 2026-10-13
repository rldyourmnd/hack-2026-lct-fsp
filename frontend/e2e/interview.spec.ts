import { expect, test } from '@playwright/test';
import { ALEXEY, resetMock, signIn } from './support';

declare global {
  interface Window {
    __fspSpeechTest: {
      failStart: boolean;
      stoppedTracks: number;
      late: () => void;
    };
  }
}

test('microphone and transcription failures preserve drafts and released recordings cannot change an answer', async ({
  page,
}) => {
  await resetMock('baseline');
  await page.addInitScript(() => {
    let previous: Recorder | null = null;
    class Recorder {
      static isTypeSupported(type: string) {
        return type === 'audio/webm;codecs=opus';
      }
      mimeType = 'audio/webm;codecs=opus';
      state = 'inactive';
      ondataavailable: ((event: { data: Blob }) => void) | null = null;
      onstop: (() => void) | null = null;
      start() {
        this.state = 'recording';
      }
      stop() {
        this.state = 'inactive';
        this.ondataavailable?.({ data: new Blob(['synthetic interview audio'], { type: this.mimeType }) });
        previous = this;
        queueMicrotask(() => this.onstop?.());
      }
    }
    Object.defineProperty(window, 'MediaRecorder', { value: Recorder, configurable: true });
    Object.defineProperty(navigator.mediaDevices, 'getUserMedia', {
      configurable: true,
      value: async () => {
        if (window.__fspSpeechTest.failStart) throw new DOMException('Denied', 'NotAllowedError');
        return { getTracks: () => [{ stop: () => window.__fspSpeechTest.stoppedTracks++ }] };
      },
    });
    window.__fspSpeechTest = {
      failStart: true,
      stoppedTracks: 0,
      late: () => previous?.onstop?.(),
    };
  });
  await signIn(page, ALEXEY, 'candidate');
  await page.goto('/candidate/assessments');
  await page.getByRole('button', { name: 'Начать собеседование' }).click();
  await page.waitForURL(/\/candidate\/interviews\//);
  await page.getByRole('button', { name: /Ответить голосом/ }).click();
  await expect(page.getByText(/Запись отправляется на сервер для распознавания GigaAM/)).toBeVisible();
  if (process.env.SCREENSHOTS) {
    await page.screenshot({ path: 'docs/screenshots/22-graph-interview.png', fullPage: true, animations: 'disabled' });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.reload();
    await page.getByRole('button', { name: /Ответить голосом/ }).click();
    await expect
      .poll(() => page.locator('#app-navigation').evaluate((navigation) => navigation.getBoundingClientRect().right))
      .toBeLessThanOrEqual(0);
    await page.evaluate(() => window.scrollTo(0, 0));
    await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(0);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    await page.screenshot({
      path: 'docs/screenshots/22b-graph-interview-mobile.png',
      fullPage: true,
      animations: 'disabled',
    });
    await page.setViewportSize({ width: 1440, height: 900 });
  }
  await page.getByRole('button', { name: 'Начать ответ', exact: true }).click();
  await expect(page.getByText(/Не удалось включить микрофон/)).toBeVisible();
  const answer = page.getByLabel('Расшифровка ответа');
  await answer.fill('Проверю логи.');
  await page.evaluate(() => {
    window.__fspSpeechTest.failStart = false;
  });
  let transcriptionCount = 0;
  let finishTranscription: (() => void) | undefined;
  const transcriptionPending = new Promise<void>((resolve) => {
    finishTranscription = resolve;
  });
  await page.route('**/api/v1/me/interviews/*/transcription', async (route) => {
    transcriptionCount++;
    expect(route.request().postDataJSON()).toMatchObject({
      questionId: expect.any(String),
      audioBase64: Buffer.from('synthetic interview audio').toString('base64'),
      mimeType: 'audio/webm',
    });
    if (transcriptionCount === 1) {
      await route.fulfill({
        status: 503,
        json: { code: 'speech_unavailable', message: 'Speech unavailable', requestId: 'test', details: [] },
      });
      return;
    }
    await transcriptionPending;
    await route.fulfill({ json: { text: 'Воспроизведу проблему.', engine: 'gigaam-v3-ctc' } });
  });
  await page.getByRole('button', { name: 'Продолжить ответ', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Отправить ответ' })).toBeDisabled();
  await page.getByRole('button', { name: 'Остановить запись' }).click();
  await expect(page.getByText(/Не удалось распознать запись/)).toBeVisible();
  await expect(answer).toHaveValue('Проверю логи.');
  await page.getByRole('button', { name: 'Продолжить ответ', exact: true }).click();
  await page.getByRole('button', { name: 'Остановить запись' }).click();
  await expect(page.getByRole('button', { name: 'Расшифровываем…', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Отправить ответ' })).toBeDisabled();
  finishTranscription?.();
  const draft = 'Проверю логи. Воспроизведу проблему.';
  await expect(answer).toHaveValue(draft);
  expect(await page.evaluate(() => window.__fspSpeechTest.stoppedTracks)).toBe(2);

  let respond: (() => void) | undefined;
  const pending = new Promise<void>((resolve) => {
    respond = resolve;
  });
  await page.route('**/api/v1/me/interviews/*/answers', async (route) => {
    await pending;
    await route.fulfill({
      status: 503,
      contentType: 'application/json',
      body: JSON.stringify({
        code: 'interview_model_unavailable',
        message: 'Модель временно недоступна',
        requestId: 'test',
        details: [],
      }),
    });
  });
  const request = page.waitForRequest('**/api/v1/me/interviews/*/answers');
  await page.getByRole('button', { name: 'Отправить ответ' }).click();
  expect((await request).postDataJSON()).toMatchObject({ answer: draft, responseMode: 'voice' });
  await expect(answer).toBeDisabled();
  await expect(page.getByRole('button', { name: /Ответить текстом/ })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Продолжить ответ', exact: true })).toBeDisabled();
  await page.evaluate(() => window.__fspSpeechTest.late());
  await expect(answer).toHaveValue(draft);
  respond?.();
  await expect(page.getByText('Модель временно недоступна')).toBeVisible();
  await expect(answer).toHaveValue(draft);
  await page.unroute('**/api/v1/me/interviews/*/answers');
  await page.getByRole('button', { name: /Ответить текстом/ }).click();
  await page.getByRole('button', { name: 'Отправить ответ' }).click();
  await expect(page.getByText(/Как сформировать и ограничить область действия ключа операции/)).toBeVisible();
  await page.evaluate(() => window.__fspSpeechTest.late());
  await expect(page.getByLabel('Ваш ответ')).toHaveValue('');
  expect(transcriptionCount).toBe(2);
});
