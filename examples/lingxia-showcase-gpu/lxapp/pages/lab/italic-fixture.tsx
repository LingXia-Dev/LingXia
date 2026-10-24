import { useLocal, type Env } from '@lingxia/ui';
import '../../theme/index.css';
export function italicFixture(env: Env) {
  const italic = useLocal('italic-enabled', false);
  const sample = 'Italic sample ffi';
  const row = 'w-[280px] h-11 text-[28px] leading-10';
  return <column className="bg-white text-black p-4 gap-4" style={{ paddingTop: env.insets.top + 16 }}>
    <button testId="italic-toggle" onClick={() => italic.set(!italic.value)} className="h-12 bg-blue-600 text-white">Toggle font style</button>
    <text testId="italic-normal" className={`${row} not-italic`}>{sample}</text>
    <text testId="italic-direct" className={`${row} italic`}>{sample}</text>
    <column className="italic"><text testId="italic-inherited" className={row}>{sample}</text></column>
    <column className="italic"><text testId="italic-reset" className={`${row} not-italic`}>{sample}</text></column>
    <text testId="italic-dynamic" className={`${row} ${italic.value ? 'italic' : 'not-italic'}`}>{sample}</text>
  </column>;
}
