import { expect, test } from '@playwright/test';
import { COMPANY_A, resetMock, signIn } from './support';

test.beforeEach(async ({ page }) => {
  await resetMock('qualified');
  await signIn(page, COMPANY_A, 'employer');
});

test('UI-16 salary validation, read-only tax basis and saving without publication', async ({ page }) => {
  await page.goto('/employer/needs/new');
  await expect(page.getByText('До вычета налогов').first()).toBeVisible();
  await expect(page.getByRole('button', { name: /Опубликовать/ })).toHaveCount(0);
  await page.getByLabel('Название').fill('Бэкенд на Rust');
  await page.getByLabel('Краткое описание').fill('Сервисы заказов.');
  await page.getByLabel('От', { exact: true }).fill('260000');
  await page.getByLabel('До', { exact: true }).fill('200000.5');
  await page.getByLabel('Навык', { exact: true }).selectOption('rust');
  await page.getByRole('button', { name: 'Сохранить и найти кандидатов' }).click();
  await expect(page.getByText('Введите целое положительное число рублей без копеек')).toBeVisible();
  await page.getByLabel('До', { exact: true }).fill('200 000');
  await page.getByRole('button', { name: 'Сохранить и найти кандидатов' }).click();
  await expect(page.getByText('Максимум не может быть меньше минимума')).toBeVisible();
  await page.getByLabel('От', { exact: true }).fill('0');
  await page.getByRole('button', { name: 'Сохранить и найти кандидатов' }).click();
  await expect(page.getByText('Сумма должна быть больше нуля')).toBeVisible();
  await page.getByLabel('От', { exact: true }).fill('180 000');
  await page.getByRole('button', { name: 'Сохранить и найти кандидатов' }).click();
  await page.waitForURL(/\/employer\/candidates\?.*mode=recommended/);
  await expect(page.getByRole('heading', { name: 'Найденные кандидаты' })).toBeVisible();
});

test('UI-17 UI-19 filters live in the URL and the shortlist survives filtering and reload', async ({ page }) => {
  await page.goto('/employer/candidates?needId=need-001&mode=recommended');
  const results = page.getByRole('table');
  await expect(results.getByRole('link', { name: 'Кандидат №018' })).toBeVisible();
  await expect(results.getByRole('button', { name: /Удалить Кандидат №018 из подборки/ })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(page.getByText('Порядок: по соответствию потребности · задаёт сервер')).toBeVisible();

  await page.getByLabel('Стек').selectOption('postgresql');
  await expect(page).toHaveURL(/skill=postgresql/);
  await expect(results.getByRole('link', { name: 'Кандидат №018' })).toHaveCount(0);
  const panel = page.getByRole('complementary', { name: 'Сохранённая подборка' });
  await expect(panel.getByRole('link', { name: 'Кандидат №018' })).toBeVisible();

  await results.getByRole('button', { name: /Добавить Кандидат №031/ }).click();
  await expect(panel.getByRole('link', { name: 'Кандидат №031' })).toBeVisible();
  await page.reload();
  await expect(
    page.getByRole('table').getByRole('button', { name: /Удалить Кандидат №031 из подборки/ }),
  ).toHaveAttribute('aria-pressed', 'true');

  await page.goBack();
  await expect(page).not.toHaveURL(/skill=postgresql/);
  await expect(page.getByLabel('Стек')).toHaveValue('');
  await page.goto('/employer/candidates?page=-4&grade=wizard&fsp=maybe');
  await expect(page.getByRole('heading', { name: 'Найденные кандидаты' })).toBeVisible();
  await expect(page.getByLabel('Грейд', { exact: true })).toHaveValue('');
});

test('UI-18 required coverage, verification gaps and the verified grade stay distinct', async ({ page }) => {
  await page.goto('/employer/candidates?needId=need-001&mode=recommended');
  const row018 = page.getByRole('row', { name: /Кандидат №018/ });
  await expect(row018.getByText('Подтверждено тестом')).toBeVisible();
  await expect(row018.getByText(/PostgreSQL: Пока нет достаточных свидетельств/)).toBeVisible();
  const row024 = page.getByRole('row', { name: /Кандидат №024/ });
  await expect(row024.getByText('2 из 2 обязательных')).toBeVisible();
  await expect(row024.getByText(/Желательно проверить: Docker, Redis/)).toBeVisible();
});

test('UI-26 publication lives on the needs list; no chat or ATS controls are offered', async ({ page }) => {
  for (const path of [
    '/employer/needs',
    '/employer/needs/need-001',
    '/employer/candidates/candidate024?needId=need-001',
  ]) {
    await page.goto(path);
    await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
    await expect(page.getByRole('button', { name: /Откликнуться|Написать|Чат|ATS/ })).toHaveCount(0);
    await expect(page.getByRole('button', { name: /Опубликовать/ })).toHaveCount(path === '/employer/needs' ? 1 : 0);
  }
});

test('search failure is an error state with retry, never a fallback result', async ({ page }) => {
  await resetMock('api_error');
  await signIn(page, COMPANY_A, 'employer');
  await page.goto('/employer/candidates');
  await expect(page.getByText('Сервис временно недоступен')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Повторить' })).toBeVisible();
  await expect(page.getByRole('table')).toHaveCount(0);
});

test('an emptied bank keeps saved shortlist entries as unavailable', async ({ page }) => {
  await resetMock('empty_bank');
  await signIn(page, COMPANY_A, 'employer');
  await page.goto('/employer/candidates');
  await expect(page.getByText('Кандидаты не найдены')).toBeVisible();
  await expect(page.getByText('Профиль сейчас недоступен').first()).toBeVisible();
});

test('graphs load on demand and a failed employer graph exposes retry instead of a hidden list fallback', async ({
  page,
}) => {
  const graphPath = '**/api/v1/candidates/candidate018/graph';
  let requests = 0;
  let release: (() => void) | undefined;
  const blocked = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route(graphPath, async (route) => {
    requests += 1;
    await blocked;
    await route.fulfill({
      status: 502,
      contentType: 'application/json',
      body: JSON.stringify({
        code: 'bifrost_unavailable',
        message: 'Сервис анализа временно недоступен',
        requestId: 'graph-test',
        details: [],
      }),
    });
  });
  await page.goto('/employer/candidates/candidate018?tab=skills');
  await expect(page.getByText('Rust', { exact: true }).first()).toBeVisible();
  expect(requests).toBe(0);
  await page.getByRole('button', { name: 'Граф', exact: true }).click();
  await expect(page.getByText('Строим граф…')).toBeVisible();
  release?.();
  await expect(page.getByText('Сервис временно недоступен')).toBeVisible();
  await expect(page.getByText('Пока граф недоступен, используйте список.')).toBeVisible();
  await expect(page.getByText('Rust', { exact: true })).toHaveCount(0);
  expect(requests).toBe(1);
  await page.unroute(graphPath);
  await page.getByRole('button', { name: 'Повторить', exact: true }).click();
  await expect(page.getByRole('button', { name: /Технология: Rust/ })).toBeVisible();
  await expect(page.getByText('Сервис временно недоступен')).toHaveCount(0);
  await page.getByRole('button', { name: 'Список', exact: true }).click();
  await expect(page.getByText('Rust', { exact: true }).first()).toBeVisible();
});
