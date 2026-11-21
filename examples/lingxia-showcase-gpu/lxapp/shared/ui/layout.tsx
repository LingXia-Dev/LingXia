import { t, useView, type Child } from '@lingxia/view';

/** Authored `data-*` props: page state automation reads as attributes. */
export type DataAttributes = Record<`data-${string}`, string>;

/**
 * A scrolling page inside the safe area; `<page>` owns insets and the keyboard.
 * No body title: the nav bar already shows it. `intro` is one line on what the page does.
 */
export function Page({
  testId,
  data,
  scrollId = 'page-scroll',
  intro,
  children,
}: {
  testId?: string;
  data?: DataAttributes;
  scrollId?: string;
  intro?: string;
  children?: Child;
}) {
  const { insets, safeArea, keyboard } = useView().env;
  // A see-through tab bar sits over the page's bottom edge: its labels need a plate to read on
  // while rows pass under them.
  const bar = keyboard.height > 0 ? 0 : insets.bottom - safeArea.bottom;
  return (
    <page data-testid={testId} {...data} className="bg-canvas text-text">
      <scroll data-testid={scrollId} className="grow basis-0">
        <view className="w-full max-w-3xl self-center px-4 pt-5 pb-10 gap-4">
          {intro ? <text className="text-sm text-muted text-center line-clamp-3">{intro}</text> : null}
          {children}
        </view>
      </scroll>
      {bar > 0 ? (
        <view aria-hidden data-testid="page-bar-plate" style={{ height: insets.bottom }}
          className="absolute left-0 right-0 bottom-0 bg-surface/95 border-t border-line" />
      ) : null}
    </page>
  );
}

export function Header({ title, subtitle, testId }: { title: string; subtitle?: string; testId?: string }) {
  return (
    <view className="items-center gap-2 pb-1">
      <text data-testid={testId} role="heading" headingLevel={1} className="text-2xl font-light text-center">
        {title}
      </text>
      {subtitle ? <text className="text-sm text-muted text-center line-clamp-3">{subtitle}</text> : null}
    </view>
  );
}

/** A titled group of related rows or controls. */
export function Section({
  title,
  description,
  testId,
  children,
  action,
}: {
  title: string;
  description?: string;
  testId?: string;
  children?: Child;
  action?: Child;
}) {
  return (
    <view data-testid={testId} className="bg-surface text-text rounded-2xl border border-line shadow-sm">
      <view className="flex-row items-center gap-3 px-4 py-4 border-b border-line">
        <view className="grow basis-0 min-w-0 gap-0.5">
          <text role="heading" headingLevel={2} className="text-sm font-semibold">{title}</text>
          {description ? <text className="text-xs text-muted line-clamp-4">{description}</text> : null}
        </view>
        {action}
      </view>
      <view className="p-4 gap-3">{children}</view>
    </view>
  );
}

export function Card({ testId, className = 'p-4 gap-3', children }: { testId?: string; className?: string; children?: Child }) {
  return (
    <view data-testid={testId} className={`bg-surface text-text rounded-2xl border border-line shadow-sm ${className}`}>
      {children}
    </view>
  );
}

const display = (value: unknown) =>
  value === undefined || value === null || value === ''
    ? '--'
    : Array.isArray(value)
      ? value.length
        ? value.join('\n')
        : '--'
      : typeof value === 'object'
        ? JSON.stringify(value, null, 2)
        : String(value);

/** One labeled value; rows separate with a divider. */
export function InfoRow({ label, value, testId }: { label: string; value: unknown; testId?: string }) {
  return (
    <view className="flex-row items-start justify-between gap-3 py-2.5">
      <text className="shrink-0 text-sm text-muted">{label}</text>
      <text data-testid={testId} className="grow basis-0 min-w-0 text-right text-sm font-semibold break-all whitespace-pre-wrap">
        {display(value)}
      </text>
    </view>
  );
}

export function InfoList({ testId, children }: { testId?: string; children?: Child }) {
  return (
    <view data-testid={testId} className="divide-y divide-line">
      {children}
    </view>
  );
}

/** Every field of a result object, or a placeholder before there is one. */
export function Details({ value, testId }: { value: unknown; testId: string }) {
  if (value === null || value === undefined)
    return (
      <text data-testid={testId} className="text-sm text-muted">
        No result yet.
      </text>
    );
  const entries: [string, unknown][] =
    typeof value === 'object' && !Array.isArray(value) ? Object.entries(value as Record<string, unknown>) : [[t('common.result'), value]];
  return (
    <InfoList testId={testId}>
      {entries.map(([name, item]) => (
        <InfoRow key={name} label={name} value={item} testId={`${testId}-${name}`} />
      ))}
    </InfoList>
  );
}
