import { cx, useLocal } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { Button } from '../Button/Button';
import { Glyph } from '../internal/Glyph';
import { FirstLine } from '../internal/firstLine';
import { bannerRecipe as r } from './Banner.recipe';
import { dataOf } from '../internal/data';

export type BannerTone = 'neutral' | 'info' | 'success' | 'warning' | 'danger';

const toneGlyph = { neutral: 'info', info: 'info', success: 'success', warning: 'warning', danger: 'error' } as const;

export type BannerDismiss =
  | {
      /** Removes the banner; the app stops rendering it. */
      onDismiss: () => void;
      /** The dismiss button's name: "Dismiss". Defaults to "Dismiss" in the page's language. */
      dismissLabel?: string;
    }
  | { onDismiss?: never; dismissLabel?: never };

export type BannerCollapse =
  | {
      /** Folds the message and the action under the title, behind a chevron button. @default false */
      isCollapsible: true;
      /** The chevron button's name: "Details". Read with whether it is open. Defaults to "Details" in the page's language. */
      toggleLabel?: string;
      /** Whether it starts unfolded. @default true */
      isDefaultOpen?: boolean;
    }
  | { isCollapsible?: false; toggleLabel?: never; isDefaultOpen?: never };

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
export function Banner({ title, message, tone = 'info', action, onDismiss, dismissLabel = liuliText('@liuli.dismiss'), isCollapsible = false, toggleLabel = liuliText('@liuli.details'), isDefaultOpen = true, className, 'data-testid': testId, ...attributes }: BannerProps) {
  const open = useLocal('open', isDefaultOpen);
  const isOpen = !isCollapsible || open.value;
  const hasEnd = onDismiss !== undefined || isCollapsible;
  return (
    <view
      role={tone === 'danger' ? 'alert' : 'status'}
      data-testid={testId} {...dataOf(attributes)}
      className={cx(r.root.base, r.root.tone[tone], !hasEnd && r.root.plain, className)}
    >
      <FirstLine strut={r.title.base}><Glyph name={toneGlyph[tone]} side={20} className={cx(r.icon.base, r.icon.tone[tone])} /></FirstLine>
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
          onClick={() => open.set(!open.value)}
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
