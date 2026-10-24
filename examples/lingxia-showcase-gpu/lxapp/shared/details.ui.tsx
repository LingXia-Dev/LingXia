import { type Env } from '@lingxia/ui';
import { colors } from './screen.ui';

export function Details({ value, testId, env }: { value: unknown; testId: string; env: Env }) {
  const c = colors(env);
  if (value === null || value === undefined) return <text testId={testId} style={{ color: c.muted, fontSize: 15 }}>No result yet.</text>;
  const entries: [string, unknown][] = typeof value === 'object' && !Array.isArray(value)
    ? Object.entries(value as Record<string, unknown>)
    : [['Result', value]];
  return <column style={{ background: c.surface, padding: 16, radius: 16, gap: 12 }}>
    {entries.map(([name, item]) => <column key={name} style={{ gap: 4 }}>
      <text style={{ color: c.muted, fontSize: 13 }}>{name}</text>
      <text testId={`${testId}-${name}`} style={{ color: c.ink, fontSize: 16, lineHeight: 23 }}>{typeof item === 'object' ? JSON.stringify(item, null, 2) : String(item ?? '—')}</text>
    </column>)}
  </column>;
}
