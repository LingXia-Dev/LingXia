import { cx, useLocal } from '@lingxia/view';
import { Button } from '../Button/Button';
import { Glyph } from '../internal/Glyph';
import { bannerRecipe as r } from './Banner.recipe';

export type BannerTone = 'neutral' | 'info' | 'success' | 'warning' | 'danger';

const toneGlyph = { neutral: 'info', info: 'info', success: 'success', warning: 'warning', danger: 'error' } as const;

export type BannerDismiss =
  | {
      /** Removes the banner; the app stops rendering it. */
      onDismiss: () => void;
      /** The dismiss button's name: "Dismiss". */
      dismissLabel: string;
    }
  | { onDismiss?: never; dismissLabel?: never };

export type BannerCollapse =
  | {
      /** Folds the message and the action under the title, behind a chevron button. @default false */
      isCollapsible: true;
      /** The chevron button's name: "Details". Read with whether it is open. */
      toggleLabel: string;
      /** Whether it starts folded. @default false */
      isDefaultCollapsed?: boolean;
    }
  | { isCollapsible?: false; toggleLabel?: never; isDefaultCollapsed?: never };

export type BannerProps = BannerDismiss & BannerCollapse & {
  /** The point, in a few words. */
  title: string;
  /** What it means or what to do, in a sentence. */
  message?: string;
  /** @default 'info' */
  tone?: BannerTone;
  /** One action that resolves it: "Update card", "Retry". */
  action?: { label: string; onClick: () => void };
  className?: string;
  'data-testid'?: string;
};

/**
 * A message in the page's flow about the page or the account: a sync problem, a trial ending, a
 * change that saved. It stays until resolved or dismissed; it is not a toast.
 */
export function Banner({ title, message, tone = 'info', action, onDismiss, dismissLabel, isCollapsible = false, toggleLabel, isDefaultCollapsed = false, className, 'data-testid': testId }: BannerProps) {
  const collapsed = useLocal('collapsed', isDefaultCollapsed);
  const isOpen = !isCollapsible || !collapsed.value;
  const hasEnd = onDismiss !== undefined || isCollapsible;
  return (
    <view
      role={tone === 'danger' ? 'alert' : 'status'}
      data-testid={testId}
      className={cx(r.root.base, r.root.tone[tone], !hasEnd && r.root.plain, className)}
    >
      <Glyph name={toneGlyph[tone]} className={cx(r.icon.base, r.icon.tone[tone])} />
      <view className={r.text.base}>
        <text className={r.title.base}>{title}</text>
        {isOpen && message !== undefined && <text className={r.message.base}>{message}</text>}
        {isOpen && action !== undefined && (
          <Button label={action.label} onClick={action.onClick} variant="link" size="sm" className={r.action.base} data-testid={testId && `${testId}-action`} />
        )}
      </view>
      {isCollapsible && (
        <button
          aria-label={toggleLabel}
          aria-expanded={isOpen}
          onClick={() => collapsed.set(!collapsed.value)}
          data-testid={testId && `${testId}-toggle`}
          className={r.dismiss.base}
        >
          <Glyph name={isOpen ? 'chevron-up' : 'chevron-down'} className={r.dismissIcon.base} />
        </button>
      )}
      {onDismiss !== undefined && (
        <button aria-label={dismissLabel} onClick={onDismiss} data-testid={testId && `${testId}-dismiss`} className={r.dismiss.base}>
          <Glyph name="close" className={r.dismissIcon.base} />
        </button>
      )}
    </view>
  );
}
