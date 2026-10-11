import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { CategoryBadge, TargetGradeBadge, VerifiedGradeBadge } from './ui';

describe('qualification badges', () => {
  it('never shows a confirmed grade without a verified one', () => {
    render(<VerifiedGradeBadge grade={null} />);
    expect(screen.getByText('Уровень ещё не подтверждён')).toBeTruthy();
    expect(screen.queryByText(/^(Джуниор|Мидл|Сеньор) подтверждён/)).toBeNull();
  });

  it('keeps the selected target separate from the verified grade', () => {
    render(
      <>
        <VerifiedGradeBadge grade="junior" />
        <TargetGradeBadge grade="middle" />
      </>,
    );
    expect(screen.getByText('Джуниор подтверждён')).toBeTruthy();
    expect(screen.getByText(/Цель: Мидл/)).toBeTruthy();
  });

  it('says where a category comes from and never calls an estimate confirmed', () => {
    render(
      <>
        <CategoryBadge specialization="onec" grade="middle" source="estimate" confidence="high" />
        <CategoryBadge specialization="backend" grade="senior" source="assessment" confidence={null} />
        <CategoryBadge specialization={null} grade={null} source={null} confidence={null} />
      </>,
    );
    expect(screen.getByText('1С · Мидл · оценка, высокая уверенность')).toBeTruthy();
    expect(screen.getByText('Бэкенд · Сеньор · подтверждено тестом')).toBeTruthy();
    expect(screen.getByText('Категория не присвоена')).toBeTruthy();
  });
});
