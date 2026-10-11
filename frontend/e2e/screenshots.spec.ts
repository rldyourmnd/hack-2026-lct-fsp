import { expect, type Page, test } from '@playwright/test';
import { answerKnowledgeTest } from './knowledge';
import { ALEXEY, actor, COMPANY_A, resetMock, signIn } from './support';

// Regenerates the documentation screenshots; runs only when SCREENSHOTS=1.
test.skip(!process.env.SCREENSHOTS, 'Set SCREENSHOTS=1 to refresh docs/screenshots');

const shot = async (page: Page, name: string) => {
  // Form actions can scroll to the last question; capture fixed chrome at the page top.
  await page.evaluate(() => window.scrollTo({ top: 0, behavior: 'instant' }));
  await page.waitForTimeout(400);
  await page.screenshot({ path: `docs/screenshots/${name}.png`, fullPage: true });
};

test('documentation screenshots of the implemented screens', async ({ browser, page }) => {
  test.setTimeout(240_000);
  await resetMock('baseline');
  await page.goto('/login');
  await expect(page.getByRole('heading', { name: 'Вход и регистрация' })).toBeVisible();
  await shot(page, '00-login');
  await signIn(page, ALEXEY, 'candidate');
  await expect(page.getByText('Бэкенд · Джуниор · подтверждено тестом').first()).toBeVisible();
  await shot(page, '01-candidate-overview');
  await page.goto('/candidate/profile');
  await shot(page, '02-profile-goals');
  await page.goto('/candidate/sources');
  await expect(page.getByRole('heading', { name: 'Отчёт GitHub' })).toBeVisible();
  await shot(page, '03-profile-sources');
  await page.goto('/candidate/skills?view=graph&node=skill-postgresql');
  await expect(page.getByRole('button', { name: /Технология: PostgreSQL/ })).toBeVisible();
  await shot(page, '04-skills-evidence');
  await page.goto('/candidate/overview');
  await page.getByRole('button', { name: 'Начать проверку' }).click();
  await page.waitForURL(/\/workspace$/);
  await answerKnowledgeTest(page);
  await shot(page, '05-assessment-workspace');
  await page.getByRole('button', { name: 'Отправить ответы' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Отправить' }).click();
  await expect(page.getByText('Профиль обновлён автоматически')).toBeVisible();
  await shot(page, '06-assessment-result');

  const employer = await actor(browser, COMPANY_A, 'employer');
  await employer.page.goto('/employer/needs/need-001');
  await shot(employer.page, '07-employer-need');
  await employer.page.goto('/employer/candidates?needId=need-001&mode=recommended');
  await expect(employer.page.getByRole('table')).toBeVisible();
  await shot(employer.page, '08-candidate-search');
  await employer.page.goto('/employer/candidates/candidate024?needId=need-001');
  await shot(employer.page, '09-candidate-employer-view');
  await employer.page.getByRole('button', { name: 'Пригласить кандидата' }).click();
  await employer.page
    .getByLabel('Описание предложения')
    .fill('Приглашаем обсудить роль бэкенд-разработчика в команде сервисов заказов.');
  await employer.page.screenshot({ path: 'docs/screenshots/09b-invitation-composer.png' });
  await employer.page.getByRole('dialog').getByRole('button', { name: 'Отправить приглашение' }).click();
  await expect(employer.page.getByText('Ждём решения кандидата.', { exact: false })).toBeVisible();

  await page.goto('/candidate/invitations');
  await page.getByRole('link', { name: /Север Тех/ }).click();
  await expect(page.getByText('Что откроется после принятия')).toBeVisible();
  await shot(page, '10-candidate-invitation');
  await page.getByRole('button', { name: 'Принять и открыть контакты' }).click();
  await expect(page.getByRole('heading', { name: 'Вы приняли приглашение' })).toBeVisible();
  await shot(page, '11-invitation-accepted');
  await employer.page.reload();
  await expect(employer.page.getByText('Контакты, открытые кандидатом')).toBeVisible();
  await shot(employer.page, '12-employer-disclosed-contacts');
  await employer.page.goto('/employer/invitations');
  await shot(employer.page, '13-employer-invitations');
  await employer.page.goto('/employer/needs');
  await shot(employer.page, '17-employer-needs');
  await page.goto('/candidate/vacancies');
  await expect(page.getByText(/Найдено: \d/)).toBeVisible();
  await shot(page, '18-candidate-vacancies');
  await page.goto('/candidate/vacancies/need-002');
  await page
    .getByLabel('Сопроводительное письмо')
    .fill('Хочу развивать сервис уведомлений: интересны очереди, повторные попытки и наблюдаемость.');
  await shot(page, '19-candidate-vacancy');
  await page.getByRole('button', { name: 'Откликнуться и открыть контакты' }).click();
  await page.waitForURL(/\/candidate\/applications\/application-/);
  await shot(page, '20-candidate-application');
  await employer.page.goto('/employer/applications');
  await shot(employer.page, '21-employer-applications');
  await employer.page.getByRole('link', { name: 'Кандидат №024' }).click();
  await expect(employer.page.getByRole('heading', { name: 'Отклик: Кандидат №024' })).toBeVisible();
  await shot(employer.page, '22-employer-application');
  await page.goto('/candidate/tasks');
  await page
    .getByLabel('Ваш подход')
    .fill('Очередь попыток в PostgreSQL со временем следующей попытки, экспоненциальная задержка и предел попыток.');
  await shot(page, '23-candidate-tasks');
  await page.getByRole('button', { name: 'Отправить ответ' }).click();
  await expect(page.getByText('Ждёт оценки компании')).toBeVisible();
  await employer.page.goto('/employer/tasks?tab=responses');
  await shot(employer.page, '24-employer-task-responses');

  await resetMock('assessment_failure');
  const failing = await actor(browser, ALEXEY, 'candidate');
  await failing.page.getByRole('button', { name: 'Начать проверку' }).click();
  await answerKnowledgeTest(failing.page, false);
  await failing.page.getByRole('button', { name: 'Отправить ответы' }).click();
  await failing.page.getByRole('dialog').getByRole('button', { name: 'Отправить' }).click();
  await expect(failing.page.getByRole('heading', { name: 'Мидл пока не подтверждён' })).toBeVisible();
  await shot(failing.page, '14-failed-upgrade');

  const fresh = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  await signIn(fresh, `onboarding-${Date.now()}@example.com`, 'candidate');
  await shot(fresh, '15-onboarding');
  // The automatic profile: GitHub answers the questionnaire and gives an estimated category.
  await fresh.getByLabel('Согласен на обработку данных').click();
  await expect(fresh.getByLabel('Согласен на обработку данных')).toBeChecked();
  await fresh.getByRole('switch', { name: 'Показывать профиль работодателям' }).click();
  await fresh.getByLabel('GitHub: ник или ссылка на профиль').fill('octo-cat');
  await fresh.getByRole('button', { name: 'Подключить GitHub' }).click();
  await expect(fresh.getByText('Бэкенд · Мидл · оценка, высокая уверенность')).toBeVisible();
  await shot(fresh, '25-onboarding-auto-profile');
  await fresh.goto('/candidate/overview');
  await expect(fresh.getByText('Из чего складывается балл')).toBeVisible();
  await shot(fresh, '26-candidate-estimate');
  const bank = await actor(browser, COMPANY_A, 'employer');
  await bank.page.goto('/employer/candidates?categorySource=estimate');
  await expect(bank.page.getByRole('table')).toBeVisible();
  await shot(bank.page, '27-employer-estimated-candidates');
  await resetMock('api_error');
  await signIn(fresh, COMPANY_A, 'employer');
  await fresh.goto('/employer/candidates');
  await expect(fresh.getByText('Сервис временно недоступен')).toBeVisible();
  await shot(fresh, '16-search-api-error');
});
