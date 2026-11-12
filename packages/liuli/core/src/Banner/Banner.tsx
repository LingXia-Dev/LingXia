import { cx } from '@lingxia/view';
import { Button } from '../Button/Button';
import { glyphBox, glyphs } from '../internal/glyphs';
import { bannerRecipe as r } from './Banner.recipe';

export type BannerTone = 'neutral' | 'info' | 'success' | 'warning' | 'danger';

const toneGlyph = { neutral: glyphs.info, info: glyphs.info, success: glyphs.success, warning: glyphs.warning, danger: glyphs.error } as const;

export type BannerDismiss =
  | {
      /** Removes the banner; the app stops rendering it. */
      onDismiss: () => void;
      /** The dismiss button's name: "Dismiss". */
      dismissLabel: string;
    }
  | { onDismiss?: never; dismissLabel?: never };

export type BannerProps = BannerDismiss & {
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
export function Banner({ title, message, tone = 'info', action, onDismiss, dismissLabel, className, 'data-testid': testId }: BannerProps) {
  return (
    <view
      role={tone === 'danger' ? 'alert' : 'status'}
      data-testid={testId}
      className={cx(r.root.base, r.root.tone[tone], onDismiss === undefined && r.root.plain, className)}
    >
      <vector aria-hidden paths={toneGlyph[tone]} viewBox={glyphBox} className={cx(r.icon.base, r.icon.tone[tone])} />
      <view className={r.text.base}>
        <text className={r.title.base}>{title}</text>
        {message !== undefined && <text className={r.message.base}>{message}</text>}
        {action !== undefined && (
          <Button label={action.label} onClick={action.onClick} variant="link" size="sm" className={r.action.base} data-testid={testId && `${testId}-action`} />
        )}
      </view>
      {onDismiss !== undefined && (
        <button aria-label={dismissLabel} onClick={onDismiss} data-testid={testId && `${testId}-dismiss`} className={r.dismiss.base}>
          <vector aria-hidden paths={glyphs.close} viewBox={glyphBox} className={r.dismissIcon.base} />
        </button>
      )}
    </view>
  );
}
