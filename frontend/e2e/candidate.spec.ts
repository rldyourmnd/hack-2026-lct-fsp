import { readFile } from 'node:fs/promises';
import { expect, test } from '@playwright/test';
import { answerKnowledgeTest } from './knowledge';
import { ALEXEY, latestCode, requestCode, resetMock, signIn } from './support';

async function submitCurrentAttempt(page: import('@playwright/test').Page) {
  await expect(page.getByRole('heading', { name: /Проверка/ })).toBeVisible();
  await answerKnowledgeTest(page);
  await page.getByRole('button', { name: 'Отправить ответы' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Отправить' }).click();
  await page.waitForURL(/\/result$/);
}

test('UI-02 UI-07 a new candidate fills the questionnaire by hand and confirms a category by test', async ({
  page,
}) => {
  await resetMock('baseline');
  const email = `junior-${Date.now()}@example.com`;
  await requestCode(page, email, 'candidate');
  await page.getByLabel('Код из письма').fill(await latestCode(email));
  await page.getByRole('button', { name: 'Подтвердить и войти' }).click();
  await page.waitForURL('**/candidate/onboarding');

  // Consent changes are server-confirmed, so the box flips only after the response.
  await page.getByLabel('Согласен на обработку данных').click();
  await expect(page.getByLabel('Согласен на обработку данных')).toBeChecked();
  await page.getByRole('switch', { name: 'Показывать профиль работодателям' }).click();
  await expect(page.getByRole('switch', { name: 'Показывать профиль работодателям' })).toHaveAttribute(
    'aria-checked',
    'true',
  );

  await page.getByLabel('Имя и фамилия').fill('Мария Новикова');
  await page.getByLabel('Грейд для проверки').selectOption('junior');
  await page.getByRole('combobox', { name: 'Стек' }).fill('Rust');
  await page.getByRole('option', { name: /^Rust/ }).click();
  await page.getByLabel('В IT с какого года').fill(String(new Date().getFullYear()));
  await page.getByRole('textbox', { name: 'Опыт', exact: true }).fill('Учебные проекты на Rust.');
  await page.getByLabel('Роль в команде').fill('Разработчик');
  await page.getByRole('button', { name: 'Сохранить анкету' }).click();
  await expect(page.getByText('Профиль сохранён')).toBeVisible();
  await expect(page.getByText('Указать ФСП ID')).toHaveCount(0);

  // Saving keeps the questionnaire open; without a source there is no estimate yet.
  await expect(page.getByLabel('Имя и фамилия')).toBeVisible();
  await expect(page.getByText(/Оценка появится, когда подключится источник/)).toBeVisible();

  await page.getByRole('button', { name: 'Пройти тест' }).click();
  await page.waitForURL(/\/workspace$/);
  await submitCurrentAttempt(page);
  await expect(page.getByRole('heading', { name: 'Джуниор подтверждён' })).toBeVisible();
  await expect(page.getByText('Профиль обновлён автоматически')).toBeVisible();
  await page.getByRole('link', { name: 'Открыть профиль' }).click();
  await expect(page.getByText('Бэкенд · Джуниор · подтверждено тестом').first()).toBeVisible();
  await expect(page.getByText(/Для работодателей:/)).toBeVisible();
});

test('a graph interview follows a missing prerequisite and survives reload', async ({ page }) => {
  await resetMock('baseline');
  await signIn(page, ALEXEY, 'candidate');
  await page.goto('/candidate/assessments');
  await page.getByRole('button', { name: 'Начать собеседование' }).click();
  await page.waitForURL(/\/candidate\/interviews\//);
  await expect(page.getByText(/Клиент повторяет POST-запрос/)).toBeVisible();
  await expect(page.getByLabel('Граф компетенций интервью')).toBeVisible();
  await expect(page.locator('.react-flow__edge')).not.toHaveCount(0);
  await page.getByLabel(/Ваш ответ|Расшифровка ответа/).fill('Сначала посмотрю логи и воспроизведу проблему.');
  await page.getByRole('button', { name: 'Отправить ответ' }).click();
  await expect(page.getByText(/Как сформировать и ограничить область действия ключа операции/)).toBeVisible();
  await expect(page.getByText('Определяет уникальность операции').first()).toBeVisible();
  await page.reload();
  await expect(page.getByText(/Как сформировать и ограничить область действия ключа операции/)).toBeVisible();
});

test('UI-10 UI-12 UI-05 draft survives reload, upgrade updates every view and the PDF is real', async ({ page }) => {
  await resetMock('baseline');
  await signIn(page, ALEXEY, 'candidate');
  await page.getByRole('button', { name: 'Начать проверку' }).click();
  await page.waitForURL(/\/workspace$/);
  const url = page.url();
  await page.getByRole('radio').first().check();
  await expect(page.getByText(/Черновик сохранён в/)).toBeVisible();
  await page.reload();
  await expect(page.getByRole('radio').first()).toBeChecked();
  expect(page.url()).toBe(url);
  await page.goto('/candidate/assessments');
  await expect(page.getByRole('link', { name: 'Продолжить', exact: true })).toHaveCount(1);
  await page.getByRole('link', { name: 'Продолжить', exact: true }).click();

  await submitCurrentAttempt(page);
  await expect(page.getByRole('heading', { name: 'Мидл подтверждён' })).toBeVisible();
  await expect(page.getByText('Профиль обновлён автоматически')).toBeVisible();
  await expect(page.getByText('Было:')).toContainText('Джуниор');
  await page.goto('/candidate/overview');
  await expect(page.getByText('Бэкенд · Мидл · подтверждено тестом').first()).toBeVisible();
  await page.goto('/candidate/skills?node=skill-postgresql');
  await expect(page.getByRole('complementary').getByText('Подтверждено для уровня «Мидл»')).toBeVisible();

  const downloadPromise = page.waitForEvent('download');
  await page.goto('/candidate/overview');
  await page.getByRole('button', { name: 'Скачать PDF' }).click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toBe('fsp-profile.pdf');
  const bytes = await readFile((await download.path()) as string);
  expect(bytes.subarray(0, 5).toString()).toBe('%PDF-');
});

test('UI-13 a failed upgrade keeps the verified Junior grade', async ({ page }) => {
  await resetMock('assessment_failure');
  await signIn(page, ALEXEY, 'candidate');
  await page.getByRole('button', { name: 'Начать проверку' }).click();
  await submitCurrentAttempt(page);
  await expect(page.getByRole('heading', { name: 'Мидл пока не подтверждён' })).toBeVisible();
  await expect(page.getByText(/Джуниор» остаётся в профиле/)).toBeVisible();
  await page.goto('/candidate/overview');
  await expect(page.getByText('Бэкенд · Джуниор · подтверждено тестом').first()).toBeVisible();
});

test('UI-14 a service failure is retryable and carries no penalty', async ({ page }) => {
  await resetMock('service_failure');
  await signIn(page, ALEXEY, 'candidate');
  await page.getByRole('button', { name: 'Начать проверку' }).click();
  await submitCurrentAttempt(page);
  await expect(page.getByRole('heading', { name: 'Оценка не получена' })).toBeVisible();
  await expect(page.getByText(/Штрафа и дополнительного ожидания нет/)).toBeVisible();
  await expect(page.getByRole('button', { name: /Начать новую попытку/ })).toBeVisible();
  await page.goto('/candidate/overview');
  await expect(page.getByText('Бэкенд · Джуниор · подтверждено тестом').first()).toBeVisible();
});

test('UI-15 cooldown comes from the server', async ({ page }) => {
  await resetMock('qualified');
  await signIn(page, ALEXEY, 'candidate');
  await page.goto('/candidate/profile');
  await page.getByLabel('Грейд для проверки').selectOption('senior');
  await page.getByRole('button', { name: 'Сохранить профиль' }).click();
  await expect(page.getByText('Профиль сохранён')).toBeVisible();
  await page.goto('/candidate/overview');
  await expect(page.getByRole('heading', { name: 'Смена грейда пока недоступна' })).toBeVisible();
  await expect(page.getByText(/Доступно с/)).toBeVisible();
});

test('UI-06 UI-07 GitHub connects by login with a public report; empty FSP history is valid', async ({ page }) => {
  await resetMock('baseline');
  await signIn(page, ALEXEY, 'candidate');
  await page.goto('/candidate/sources');
  await expect(page.getByRole('heading', { name: 'Отчёт GitHub' })).toBeVisible();
  await expect(page.getByText('Владение подтверждено', { exact: false })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Портрет разработчика' })).toBeVisible();
  // Report sections are tabs kept in the URL, so a reload returns to the same section.
  await page.getByRole('tab', { name: 'Активность' }).click();
  await expect(page).toHaveURL(/tab=activity/);
  await page.reload();
  await expect(page.getByRole('img', { name: /Коммиты за год по дням/ })).toBeVisible();
  await page.getByRole('tab', { name: 'Для работодателя' }).click();
  await expect(
    page.getByText('Этот обезличенный блок работодатель видит в карточке кандидата', { exact: false }),
  ).toBeVisible();
  await page.getByRole('tab', { name: 'Обзор' }).click();

  await page.getByRole('button', { name: 'Подключить другой аккаунт' }).click();
  const login = page.getByLabel('Ник или ссылка на профиль GitHub');
  await login.fill('https://github.com/octo-cat/repo');
  await page.getByRole('button', { name: 'Подключить другой аккаунт' }).click();
  await expect(page.getByText('Укажите имя пользователя GitHub или ссылку на профиль')).toBeVisible();
  await login.fill('https://github.com/octo-cat');
  await page.getByRole('button', { name: 'Подключить другой аккаунт' }).click();
  // The analysis runs as a background job; the report and the employer preview arrive with it.
  await expect(page.getByText('Владение аккаунтом не подтверждено').first()).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Портрет разработчика' })).toBeVisible();
  await expect(page.getByText(/^fsp-demo/)).toBeVisible();

  await page.getByRole('button', { name: 'Указать ФСП ID' }).click();
  await page.getByLabel('ФСП ID').fill('FSP-UNKNOWN-1');
  await page.getByRole('button', { name: 'Подключить', exact: true }).click();
  await expect(page.getByText('Истории ФСП нет')).toBeVisible();
});

test('UI-06 a new candidate connects GitHub and gets the questionnaire and a category without typing', async ({
  page,
}) => {
  await resetMock('baseline');
  const email = `github-${Date.now()}@example.com`;
  await requestCode(page, email, 'candidate');
  await page.getByLabel('Код из письма').fill(await latestCode(email));
  await page.getByRole('button', { name: 'Подтвердить и войти' }).click();
  await page.waitForURL('**/candidate/onboarding');
  await expect(page.getByText('Подключение источников доступно после согласия на обработку данных.')).toBeVisible();
  await page.getByLabel('Согласен на обработку данных').click();
  await expect(page.getByLabel('Согласен на обработку данных')).toBeChecked();
  await page.getByRole('switch', { name: 'Показывать профиль работодателям' }).click();
  await page.getByLabel('GitHub: ник или ссылка на профиль').fill('@octo-cat');
  await page.getByRole('button', { name: 'Подключить GitHub' }).click();
  // The analysis job ends with the report; the estimate answers the questionnaire and gives the category.
  await expect(page.getByText(/GitHub «octo-cat» подключён, анализ завершён/)).toBeVisible();
  await expect(page.getByText('Бэкенд · Мидл · оценка, высокая уверенность')).toBeVisible();
  await expect(page.getByText('Масштаб и сложность', { exact: true }).first()).toBeVisible();
  // What raises the grade and the two answers evidence cannot give.
  await expect(page.getByRole('heading', { name: 'Как повысить грейд' })).toBeVisible();
  await expect(page.getByText('Уточните два ответа')).toBeVisible();
  // The notes say what the next grade needs; every answer can be rebuilt from the sources.
  await expect(page.getByText('Почему такой уровень')).toBeVisible();
  await expect(page.getByText(/До уровня «Сеньор» не хватает/)).toBeVisible();
  await expect(page.getByRole('button', { name: 'Заполнить анкету по источникам' })).toBeVisible();
  // The questionnaire stays open as it was and now shows the automatic answers.
  await expect(page.getByText('Проверить и поправить анкету')).toBeVisible();
  await expect(page.getByLabel('Имя и фамилия')).toHaveValue('Иван Петров');
  await expect(page.getByLabel('Грейд для проверки')).toHaveValue('middle');
  // Public tenure is not written in as the candidate's experience; the start year is asked instead.
  await expect(page.locator('#profile-since')).toHaveValue('');
  await page.locator('#refine-since').fill('2016');
  await page.getByRole('button', { name: 'Пересчитать с уточнениями' }).click();
  await expect(page.getByText('Оценка пересчитана')).toBeVisible();
  await expect(page.locator('#profile-since')).toHaveValue('2016');
  await expect(page.getByText('Заполнено автоматически.').first()).toBeVisible();

  await page.goto('/candidate/overview');
  await expect(page.getByText('Бэкенд · Мидл · оценка, высокая уверенность').first()).toBeVisible();
  await expect(page.getByText(/Для работодателей:/)).toBeVisible();
  await page.goto('/candidate/onboarding');
  await page.getByRole('link', { name: 'Подробный отчёт и подтверждение владения' }).click();
  await expect(page.getByRole('heading', { name: 'Портрет разработчика' })).toBeVisible();
});

test('UI-06 a verified FSP record appears as a labelled demo source', async ({ page }) => {
  await resetMock('baseline');
  await signIn(page, ALEXEY, 'candidate');
  await page.goto('/candidate/sources');
  await page.getByRole('button', { name: 'Указать ФСП ID' }).click();
  await page.getByLabel('ФСП ID').fill('FSP-DEMO-024');
  await page.getByRole('button', { name: 'Подключить', exact: true }).click();
  await expect(page.getByText('Подтверждённое участие')).toBeVisible();
  await expect(page.getByText('DEMO-PROTOCOL-API-2026-01', { exact: false })).toBeVisible();
});

test('UI-08 UI-09 graph and list share one selection and the list is keyboard-operable', async ({ page }) => {
  await resetMock('baseline');
  await signIn(page, ALEXEY, 'candidate');
  const graphRequests: string[] = [];
  page.on('request', (request) => {
    if (new URL(request.url()).pathname === '/api/v1/me/graph') graphRequests.push(request.url());
  });
  await page.goto('/candidate/skills');
  await expect(page.getByRole('button', { name: /^Docker/ })).toBeVisible();
  expect(graphRequests).toHaveLength(0);
  await page.getByRole('button', { name: 'Граф', exact: true }).click();
  await page.getByRole('button', { name: /Технология: PostgreSQL/ }).click();
  expect(graphRequests).toHaveLength(1);
  const inspector = page.getByRole('complementary', {
    name: 'Подробности выбранного элемента',
  });
  await expect(inspector.getByRole('heading', { name: 'PostgreSQL' })).toBeVisible();
  await expect(inspector.getByText('Для уровня «Мидл» нужна проверка')).toBeVisible();
  await page.getByRole('button', { name: 'Список' }).click();
  await expect(page.getByRole('button', { name: /PostgreSQL/, pressed: true })).toBeVisible();
  await page.getByRole('button', { name: /^Docker/ }).focus();
  await page.keyboard.press('Enter');
  await expect(inspector.getByRole('heading', { name: 'Docker' })).toBeVisible();
  // Concepts group the technologies; the inspector lists them with their level.
  await page.getByRole('button', { name: 'Граф', exact: true }).click();
  await page.getByRole('button', { name: /^Понятие: Базы данных и хранилища/ }).click();
  await expect(inspector.getByRole('heading', { name: 'Базы данных и хранилища' })).toBeVisible();
  await expect(inspector.getByText('PostgreSQL')).toBeVisible();
  await expect(page.getByRole('button', { name: /^Источник: GitHub/ })).toBeVisible();
});

test('UI-04 PDF import previews fixture data and applies it only on confirmation', async ({ page }) => {
  await resetMock('baseline');
  await signIn(page, ALEXEY, 'candidate');
  await page.goto('/candidate/profile');
  await page.getByRole('button', { name: 'Импортировать PDF' }).click();
  const dialog = page.getByRole('dialog');
  await dialog.locator('input[type=file]').setInputFiles({
    name: 'cv.txt',
    mimeType: 'text/plain',
    buffer: Buffer.from('hello'),
  });
  await expect(dialog.getByText('Выберите файл в формате PDF.')).toBeVisible();
  const pdf = Buffer.from('%PDF-1.4\n1 0 obj<<>>endobj\ntrailer<<>>\n%%EOF\n');
  await dialog.locator('input[type=file]').setInputFiles({
    name: 'cv.pdf',
    mimeType: 'application/pdf',
    buffer: pdf,
  });
  await expect(dialog.getByText('Демонстрационное извлечение', { exact: true })).toBeVisible();
  await expect(dialog.getByLabel('Стаж, месяцев')).toHaveValue('');
  await dialog.getByLabel('Имя и фамилия').fill('Алексей Морозов');
  await dialog.getByRole('button', { name: 'Применить к профилю' }).click();
  await expect(page.getByText('Данные из PDF применены к профилю')).toBeVisible();
});

test('FSP pending and empty histories stay distinct and never change the assessment grade', async ({ page }) => {
  await resetMock('baseline');
  await signIn(page, ALEXEY, 'candidate');
  await page.goto('/candidate/sources');
  const change = page.getByRole('button', { name: 'Изменить ФСП ID' });
  if (await change.count()) await change.click();
  else await page.getByRole('button', { name: 'Указать ФСП ID' }).click();
  await page.getByLabel('ФСП ID', { exact: true }).fill('FSP-DEMO-PENDING');
  await page.getByRole('button', { name: 'Подключить', exact: true }).click();
  await expect(page.getByText('Протокол ещё не опубликован')).toBeVisible();
  await expect(page.getByText('Подтверждённое участие', { exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Изменить ФСП ID' }).click();
  await page.getByLabel('ФСП ID', { exact: true }).fill('FSP-DEMO-EMPTY');
  await page.getByRole('button', { name: 'Подключить', exact: true }).click();
  await expect(page.getByText('Истории ФСП нет', { exact: true })).toBeVisible();
  await page.goto('/candidate/overview');
  await expect(page.getByText('Бэкенд · Джуниор · подтверждено тестом').first()).toBeVisible();
});
