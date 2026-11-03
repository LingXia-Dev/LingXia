import { useLocal, useView } from '@lingxia/view';

// The smallest GPU View: CSS-named inline style, one local and one handler.
export function render() {
  const { env } = useView();
  const greetings = useLocal('greetings', 0);
  const compact = env.viewport.height < 480;
  const count = greetings.value;
  return (
    <page testId="hello" style={{ padding: compact ? 20 : 28, gap: compact ? 12 : 20, backgroundColor: 'background' }}>
      <text style={{ fontSize: 14, color: 'primary', marginTop: compact ? 0 : 24 }}>LINGXIA</text>
      <text testId="greeting" style={{ fontSize: compact ? 28 : 36 }}>Hello, world!</text>
      <text style={{ fontSize: 18, color: 'textMuted', marginBottom: compact ? 0 : 12 }}>A little hello can brighten your day.</text>
      <column style={{ padding: compact ? 16 : 24, gap: 16, backgroundColor: 'surface', borderRadius: 20 }}>
        {!compact && <text style={{ fontSize: 22 }}>Ready when you are.</text>}
        <button testId="say-hello" onClick={() => greetings.set(count + 1)} style={{ padding: 18, fontSize: 18 }}>
          Say hello
        </button>
        <text testId="hello-count" style={{ color: 'textMuted', fontSize: 16 }}>
          {count === 0 ? 'Your first hello starts here.' : `Hello received ${count} ${count === 1 ? 'time' : 'times'}.`}
        </text>
      </column>
    </page>
  );
}
