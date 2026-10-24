import type { Child } from '@lingxia/ui';

/** A scrolling page inside the safe area; `<page>` owns insets and the keyboard. */
export function Page({
  testId,
  scrollId = 'page-scroll',
  title,
  subtitle,
  children,
}: {
  testId?: string;
  scrollId?: string;
  title?: string;
  subtitle?: string;
  children?: Child;
}) {
  return (
    <page testId={testId} className="bg-background text-foreground">
      <scroll testId={scrollId} className="grow basis-0">
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
      <text testId={testId} className="text-2xl font-light text-center">
        {title}
      </text>
      {subtitle ? <text className="text-sm text-muted-foreground text-center line-clamp-3">{subtitle}</text> : null}
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
    <column testId={testId} className="bg-card text-card-foreground rounded-2xl border border-border shadow-sm">
      <row className="items-center gap-3 px-4 py-4 border-b border-border">
        <column className="grow basis-0 min-w-0 gap-0.5">
          <text className="text-sm font-semibold">{title}</text>
          {description ? <text className="text-xs text-muted-foreground line-clamp-4">{description}</text> : null}
        </column>
        {action}
      </row>
      <column className="p-4 gap-3">{children}</column>
    </column>
  );
}

export function Card({ testId, className = 'p-4 gap-3', children }: { testId?: string; className?: string; children?: Child }) {
  return (
    <column testId={testId} className={`bg-card text-card-foreground rounded-2xl border border-border shadow-sm ${className}`}>
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
      <text className="shrink-0 text-sm text-muted-foreground">{label}</text>
      <text testId={testId} className="grow basis-0 min-w-0 text-right text-sm font-semibold break-all">
        {display(value)}
      </text>
    </row>
  );
}

export function InfoList({ testId, children }: { testId?: string; children?: Child }) {
  return (
    <column testId={testId} className="divide-y divide-border">
      {children}
    </column>
  );
}

/** Every field of a result object, or a placeholder before there is one. */
export function Details({ value, testId }: { value: unknown; testId: string }) {
  if (value === null || value === undefined)
    return (
      <text testId={testId} className="text-sm text-muted-foreground">
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
