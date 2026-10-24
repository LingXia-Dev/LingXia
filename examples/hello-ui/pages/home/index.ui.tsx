import { useLocal, type View } from '@lingxia/ui';

export function render({ env }: View) {
  const greetings = useLocal('greetings', 0);
  const compact = env.viewport.height < 480;
  return (
    <column testId="hello" style={{ padding: compact ? 20 : 28, gap: compact ? 12 : 20, background: 'background' }}>
      {!compact && <spacer style={{ height: 24 }} />}
      <text style={{ fontSize: 14, color: 'primary' }}>LINGXIA</text>
      <text testId="greeting" style={{ fontSize: compact ? 28 : 36 }}>Hello, world!</text>
      <text style={{ fontSize: 18, color: 'textMuted' }}>A little hello can brighten your day.</text>
      {!compact && <spacer style={{ height: 12 }} />}
      <column style={{ padding: compact ? 16 : 24, gap: 16, background: 'surface', radius: 20 }}>
        {!compact && <text style={{ fontSize: 22 }}>Ready when you are.</text>}
        <button testId="say-hello" onClick={() => greetings.set(greetings.value + 1)} style={{ padding: 18, fontSize: 18 }}>
          Say hello
        </button>
        <text testId="hello-count" style={{ color: 'textMuted', fontSize: 16 }}>
          {greetings.value === 0 ? 'Your first hello starts here.' : `Hello received ${greetings.value} ${greetings.value === 1 ? 'time' : 'times'}.`}
        </text>
      </column>
    </column>
  );
}
