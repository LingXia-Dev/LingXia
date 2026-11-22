import { cx, type Child } from '@lingxia/view';
import { sectionRecipe as r } from './Section.recipe';
import { dataOf } from '../internal/data';

export interface SectionProps {
  /** The section's heading. */
  title: string;
  /** A line under the heading: what the section is for. */
  description?: string;
  /** At the end of the heading's line: a Button or a link to see everything. */
  action?: Child;
  /** The heading's level in the page outline. @default 2 */
  headingLevel?: 2 | 3 | 4;
  children: Child;
  className?: string;
  'data-testid'?: string;
}

/** A titled part of a page: a heading, an optional line and action, then its content. */
export function Section({ title, description, action, headingLevel = 2, children, className, 'data-testid': testId, ...attributes }: SectionProps) {
  return (
    <view data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, className)}>
      <view className={r.header.base}>
        <view className={r.heading.base}>
          <text role="heading" headingLevel={headingLevel} className={cx(r.title.base, r.title.level[headingLevel])}>{title}</text>
          {description !== undefined && <text className={r.description.base}>{description}</text>}
        </view>
        {action !== undefined && <view className={cx(r.action.base, r.action.level[headingLevel])}>{action}</view>}
      </view>
      {children}
    </view>
  );
}
