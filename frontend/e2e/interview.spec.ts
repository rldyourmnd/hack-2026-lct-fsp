import { expect, test } from '@playwright/test';
import { ALEXEY, resetMock, signIn } from './support';

declare global {
  interface Window {
    __fspSpeechTest: {
      failStart: boolean;
      emit: (text: string) => void;
      late: (text: string) => void;
    };
  }
}

test('speech failures preserve the draft and detached callbacks cannot change a submitted answer', async ({ page }) => {
  await resetMock('baseline');
  await page.addInitScript(() => {
    type Result = { resultIndex: number; results: { isFinal: boolean; 0: { transcript: string } }[] };
    let active: Recognition | null = null;
    let previousResult: ((event: Result) => void) | null = null;
    const result = (text: string): Result => ({
      resultIndex: 0,
      results: [{ isFinal: true, 0: { transcript: text } }],
    });
    class Recognition {
      lang = '';
      continuous = false;
      interimResults = false;
      onresult: ((event: Result) => void) | null = null;
      onerror: ((event: { error: string }) => void) | null = null;
      onend: (() => void) | null = null;
      start() {
        if (window.__fspSpeechTest.failStart) throw new DOMException('Unavailable', 'InvalidStateError');
        active = this;
      }
      stop() {
        this.onend?.();
      }
      abort() {
        if (active === this) active = null;
      }
    }
    Object.defineProperty(window, 'SpeechRecognition', { value: Recognition, configurable: true });
    window.__fspSpeechTest = {
      failStart: true,
      emit: (text) => {
        previousResult = active?.onresult ?? null;
        active?.onresult?.(result(text));
      },
      late: (text) => previousResult?.(result(text)),
    };
  });
  await signIn(page, ALEXEY, 'candidate');
  await page.goto('/candidate/assessments');
  await page.getByRole('button', { name: 'Начать собеседование' }).click();
  await page.waitForURL(/\/candidate\/interviews\//);
  await page.getByRole('button', { name: /Ответить голосом/ }).click();
  await expect(page.getByText(/Браузер может передать звук/)).toBeVisible();
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
  await expect(page.getByText(/Не удалось включить распознавание/)).toBeVisible();
  const answer = page.getByLabel('Расшифровка ответа');
  await answer.fill('Проверю логи.');
  await page.evaluate(() => {
    window.__fspSpeechTest.failStart = false;
  });
  await page.getByRole('button', { name: 'Продолжить ответ', exact: true }).click();
  await page.evaluate(() => window.__fspSpeechTest.emit('Воспроизведу проблему.'));
  await expect(page.getByRole('button', { name: 'Отправить ответ' })).toBeDisabled();
  await page.getByRole('button', { name: 'Остановить запись' }).click();
  const draft = 'Проверю логи. Воспроизведу проблему.';
  await expect(answer).toHaveValue(draft);

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
  await page.evaluate(() => window.__fspSpeechTest.late('Поздняя расшифровка'));
  await expect(answer).toHaveValue(draft);
  respond?.();
  await expect(page.getByText('Модель временно недоступна')).toBeVisible();
  await expect(answer).toHaveValue(draft);
  await page.unroute('**/api/v1/me/interviews/*/answers');
  await page.getByRole('button', { name: /Ответить текстом/ }).click();
  await page.getByRole('button', { name: 'Отправить ответ' }).click();
  await expect(page.getByText(/Как сформировать и ограничить область действия ключа операции/)).toBeVisible();
  await page.evaluate(() => window.__fspSpeechTest.late('Ответ на предыдущий вопрос'));
  await expect(page.getByLabel('Ваш ответ')).toHaveValue('');
});
