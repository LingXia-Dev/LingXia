import { cx, type Child } from '@lingxia/view';
import { emptyStateRecipe as r } from './EmptyState.recipe';

export type EmptyStateSize = 'sm' | 'md';

export interface EmptyStateProps {
  /** What is empty, said plainly: "No notes yet". Drawn as a heading. */
  title: string;
  /** What to do about it, in one or two sentences. */
  description?: string;
  /** A picture above the title: an icon or the app's own illustration; hidden from assistive tech. Liuli draws none. */
  media?: Child;
  /** One or two Buttons under the text; the first is the way forward. */
  actions?: Child;
  /** The title's heading level in the page outline. @default 2 */
  headingLevel?: 1 | 2 | 3 | 4 | 5 | 6;
  /** `sm` sits inside a card or a list; `md` fills a page or a panel. @default 'md' */
  size?: EmptyStateSize;
  className?: string;
  'data-testid'?: string;
}

/** What a page, list or panel shows when it has nothing in it yet, and the way forward. */
export function EmptyState({ title, description, media, actions, headingLevel = 2, size = 'md', className, 'data-testid': testId }: EmptyStateProps) {
  return (
    <view data-testid={testId} className={cx(r.root.base, r.root.size[size], className)}>
      {media !== undefined && <view aria-hidden className={cx(r.media.base, r.media.size[size])}>{media}</view>}
      <view className={r.text.base}>
        <text role="heading" headingLevel={headingLevel} className={cx(r.title.base, r.title.size[size])}>{title}</text>
        {description !== undefined && <text className={cx(r.description.base, r.description.size[size])}>{description}</text>}
      </view>
      {actions !== undefined && <view className={r.actions.base}>{actions}</view>}
    </view>
  );
}
