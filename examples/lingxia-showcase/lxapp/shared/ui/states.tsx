import { errorText } from '@lingxia/view';
import { Icon, type IconName } from './icon';
import { Button } from './controls';

export function EmptyState({ icon = 'note', title, message, testId }: { icon?: IconName; title: string; message?: string; testId?: string }) {
  return (
    <view className="items-center gap-2 px-5 py-10">
      <Icon name={icon} className="w-12 h-12 text-muted" />
      <text data-testid={testId} className="text-lg font-medium text-center text-text">
        {title}
      </text>
      {message ? <text className="text-sm text-muted text-center">{message}</text> : null}
    </view>
  );
}

/** A failure the user can retry: the page's own words, or a coded error said by its code (`errorText`); a cancel shows nothing. */
export function ErrorState({ error, onRetry, testId }: { error: string | { readonly code: string } | null; onRetry?: () => unknown; testId?: string }) {
  const message = typeof error === 'string' ? error : error && errorText(error);
  if (!message) return null;
  return (
    <view className="flex-row items-start gap-3 p-4 rounded-xl bg-danger-soft border border-danger/40">
      <Icon name="alert" className="w-5 h-5 text-danger" />
      <text data-testid={testId} className="grow basis-0 min-w-0 text-sm text-danger break-words">
        {message}
      </text>
      {onRetry ? (
        <Button variant="ghost" size="sm" testId={testId && `${testId}-retry`} onClick={onRetry}>
          Retry
        </Button>
      ) : null}
    </view>
  );
}

export function Notice({ tone = 'success', children, testId }: { tone?: 'success' | 'error'; children: string; testId?: string }) {
  const success = tone === 'success';
  return (
    <view
      className={
        success
          ? 'flex-row items-start gap-3 p-4 rounded-xl bg-success-soft border border-success/40'
          : 'flex-row items-start gap-3 p-4 rounded-xl bg-danger-soft border border-danger/40'
      }
    >
      <Icon name={success ? 'success' : 'alert'} className={success ? 'w-5 h-5 text-success' : 'w-5 h-5 text-danger'} />
      <text
        data-testid={testId}
        className={success ? 'grow basis-0 min-w-0 text-sm text-success leading-relaxed' : 'grow basis-0 min-w-0 text-sm text-danger leading-relaxed'}
      >
        {children}
      </text>
    </view>
  );
}
