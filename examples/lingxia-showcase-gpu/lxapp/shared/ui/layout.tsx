import type { Child } from '@lingxia/ui';

/** A scrolling page inside the safe area; `<page>` owns insets and the keyboard. */
/** Authored `data-*` props: page state automation reads as attributes. */
export type DataAttributes = Record<`data-${string}`, string>;

export function Page({
  testId,
  data,
  scrollId = 'page-scroll',
  title,
  subtitle,
  children,
}: {
  testId?: string;
  data?: DataAttributes;
  scrollId?: string;
  title?: string;
  subtitle?: string;
  children?: Child;
}) {
  return (
    <page data-testid={testId} {...data} className="bg-canvas text-text">
      <scroll data-testid={scrollId} className="grow basis-0">
        <column className="w-full max-w-3xl self-center px-4 pt-5 pb-10 gap-4">
          {title ? <Header title={title} subtitle={subtitle} /> : null}
          {children}
        </column>
      </scroll>
    </page>
  );
}

export function Header({ title, subtitle, testId }: { title: string; subtitle?: string; testId?: string }) {
  return (
    <column className="items-center gap-2 pb-1">
      <text data-testid={testId} role="heading" headingLevel={1} className="text-2xl font-light text-center">
        {title}
      </text>
      {subtitle ? <text className="text-sm text-muted text-center line-clamp-3">{subtitle}</text> : null}
    </column>
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
    <column data-testid={testId} className="bg-surface text-text rounded-2xl border border-line shadow-sm">
      <row className="items-center gap-3 px-4 py-4 border-b border-line">
        <column className="grow basis-0 min-w-0 gap-0.5">
          <text role="heading" headingLevel={2} className="text-sm font-semibold">{title}</text>
          {description ? <text className="text-xs text-muted line-clamp-4">{description}</text> : null}
        </column>
        {action}
      </row>
      <column className="p-4 gap-3">{children}</column>
    </column>
  );
}

export function Card({ testId, className = 'p-4 gap-3', children }: { testId?: string; className?: string; children?: Child }) {
  return (
    <column data-testid={testId} className={`bg-surface text-text rounded-2xl border border-line shadow-sm ${className}`}>
      {children}
    </column>
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
    <row className="items-start justify-between gap-3 py-2.5">
      <text className="shrink-0 text-sm text-muted">{label}</text>
      <text data-testid={testId} className="grow basis-0 min-w-0 text-right text-sm font-semibold break-all whitespace-pre-wrap">
        {display(value)}
      </text>
    </row>
  );
}

export function InfoList({ testId, children }: { testId?: string; children?: Child }) {
  return (
    <column data-testid={testId} className="divide-y divide-line">
      {children}
    </column>
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
    typeof value === 'object' && !Array.isArray(value) ? Object.entries(value as Record<string, unknown>) : [['Result', value]];
  return (
    <InfoList testId={testId}>
      {entries.map(([name, item]) => (
        <InfoRow key={name} label={name} value={item} testId={`${testId}-${name}`} />
      ))}
    </InfoList>
  );
}
