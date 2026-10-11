import { expect, type Page } from '@playwright/test';
import bank from '../../backend/crates/domain/data/knowledge-v1.json' with { type: 'json' };
import additions from '../../backend/crates/domain/data/knowledge-v2-additions.json' with { type: 'json' };

export async function answerKnowledgeTest(page: Page, correct = true) {
  // Wait for the questionnaire, not an outgoing onboarding form's group.
  await expect(page.getByRole('radio')).toHaveCount(30);
  for (const question of [...bank.questions, ...additions.questions]) {
    const group = page.getByRole('group', { name: new RegExp(question.prompt.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')) });
    if (!(await group.count())) continue;
    const option = question.options.find((o) =>
      correct ? o.id === question.correctOptionId : o.id !== question.correctOptionId,
    );
    if (!option) throw new Error('Invalid test bank');
    await group.getByRole('radio', { name: option.label, exact: true }).check();
  }
  await expect(page.getByRole('radio', { checked: true })).toHaveCount(10);
  await expect(page.getByText(/Черновик сохранён в/)).toBeVisible();
}
