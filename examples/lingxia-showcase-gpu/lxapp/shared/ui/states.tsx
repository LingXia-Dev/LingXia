import type { ResourceError } from '@lingxia/ui';
import { Icon, type IconName } from './icon';
import { Button } from './controls';

export function EmptyState({ icon = 'note', title, message, testId }: { icon?: IconName; title: string; message?: string; testId?: string }) {
  return (
    <column className="items-center gap-2 px-5 py-10">
      <Icon name={icon} className="w-12 h-12 text-muted-foreground" />
      <text testId={testId} className="text-lg font-medium text-center text-foreground">
        {title}
      </text>
      {message ? <text className="text-sm text-muted-foreground text-center">{message}</text> : null}
    </column>
  );
}

/** A failure the user can retry; `error` accepts a message or a resource error. */
export function ErrorState({ error, onRetry, testId }: { error: string | ResourceError | null; onRetry?: () => unknown; testId?: string }) {
  if (!error) return null;
  const message = typeof error === 'string' ? error : error.message;
  return (
    <row className="items-start gap-3 p-4 rounded-xl bg-red-50 border border-red-200">
      <Icon name="alert" className="w-5 h-5 text-error" />
      <text testId={testId} className="grow basis-0 min-w-0 text-sm text-error break-words">
        {message}
      </text>
      {onRetry ? (
        <Button variant="ghost" size="sm" testId={testId && `${testId}-retry`} onClick={onRetry}>
          Retry
        </Button>
      ) : null}
    </row>
  );
}

export function Notice({ tone = 'success', children, testId }: { tone?: 'success' | 'error'; children: string; testId?: string }) {
  const success = tone === 'success';
  return (
    <row
      className={
        success
          ? 'items-start gap-3 p-4 rounded-xl bg-green-50 border border-green-200'
          : 'items-start gap-3 p-4 rounded-xl bg-red-50 border border-red-200'
      }
    >
      <Icon name={success ? 'success' : 'alert'} className={success ? 'w-5 h-5 text-success' : 'w-5 h-5 text-error'} />
      <text
        testId={testId}
        className={success ? 'grow basis-0 min-w-0 text-sm text-success leading-relaxed' : 'grow basis-0 min-w-0 text-sm text-error leading-relaxed'}
      >
        {children}
      </text>
    </row>
  );
}
