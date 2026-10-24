import { useLocal, type Env } from '@lingxia/ui';
import '../../theme/index.css';
export function trackingFixture(env: Env) {
  const tracking = useLocal('tracking-enabled', false);
  const sample = 'Tracking sample';
  const row = 'w-[280px] h-11 text-[28px] leading-10';
  return <column className="bg-white text-black p-4 gap-4" style={{ paddingTop: env.insets.top + 16 }}>
    <button testId="tracking-toggle" onClick={() => tracking.set(!tracking.value)} className="h-12 bg-blue-600 text-white">Toggle tracking</button>
    <text testId="tracking-normal" className={`${row} tracking-normal`}>{sample}</text>
    <text testId="tracking-direct" className={`${row} tracking-[2px]`}>{sample}</text>
    <column className="tracking-[2px]"><text testId="tracking-inherited" className={row}>{sample}</text></column>
    <column className="tracking-[2px]"><text testId="tracking-reset" className={`${row} tracking-normal`}>{sample}</text></column>
    <text testId="tracking-dynamic" className={`${row} ${tracking.value ? 'tracking-[2px]' : 'tracking-normal'}`}>{sample}</text>
  </column>;
}
