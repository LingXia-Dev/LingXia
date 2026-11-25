import { cx, type Child } from '@lingxia/view';
import { useLargeText } from '../internal/largeText';
import { sectionRecipe as r } from './Section.recipe';
import { dataOf } from '../internal/data';
import { firstLineRecipe } from '../internal/firstLine.recipe';
import { ActionEdge } from '../internal/actionEdge';

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
  // At large text a side-by-side action squeezes the heading to a word a line: it goes under it instead.
  const isLarge = useLargeText();
  const isStacked = action !== undefined && isLarge;
  return (
    <view data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, className)}>
      <view className={isStacked ? r.header.stacked : r.header.base}>
        <view className={isStacked ? r.heading.stacked : r.heading.base}>
          <text role="heading" headingLevel={headingLevel} className={cx(r.title.base, r.title.level[headingLevel])}>{title}</text>
          {description !== undefined && <text className={r.description.base}>{description}</text>}
        </view>
        {isStacked ? <ActionEdge.Provider value="start"><view className={r.action.stacked}>{action}</view></ActionEdge.Provider> : action !== undefined && (
          <view className={r.action.base}>
            <text aria-hidden className={cx(r.title.base, r.title.level[headingLevel], firstLineRecipe.strut)}>{'\u200B'}</text>
            <view className={r.actionControls.base}>{action}</view>
          </view>
        )}
      </view>
      {children}
    </view>
  );
}
