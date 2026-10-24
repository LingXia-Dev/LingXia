import { useLocal, type Env } from '@lingxia/ui';
import '../../theme/index.css';

export function percentFixture(env: Env) {
  const narrow = useLocal('percent-narrow', false);
  return <column className="bg-white p-4 gap-3 text-black" style={{ paddingTop: env.insets.top + 16 }}>
    <button testId="percent-toggle" onClick={() => narrow.set(!narrow.value)} className="w-60 h-12 bg-blue-600 text-white">{narrow.value ? 'Narrow' : 'Wide'}</button>
    <column className="p-4 border-2 border-black gap-2" style={{ width: narrow.value ? 240 : 320 }}>
      <text testId="percent-full" className="w-full h-8 bg-blue-100">Full width</text>
      <text testId="percent-half" className="w-1/2 h-8 bg-blue-100">Half width</text>
      <text testId="percent-min" className="w-[10%] min-w-[50%] h-8 bg-blue-100">Minimum</text>
      <text testId="percent-max" className="w-full max-w-[72%] h-8 bg-blue-100">Maximum width</text>
      <row className="justify-between items-start gap-3">
        <text className="shrink-0 text-xs">IPv6</text>
        <column className="grow basis-0 items-end max-w-[72%]" style={{ minWidth: 0 }}>
          <text testId="percent-ip" className="break-all text-right text-sm bg-blue-100">2001:db8:1234:5678:90ab:cdef:1234:5678</text>
        </column>
      </row>
      <column className="h-20 w-full">
        <text testId="percent-height" className="h-1/2 w-full shrink-0 bg-green-100">Half height</text>
        <text testId="percent-max-height" className="h-full max-h-[50%] w-full shrink-0 bg-blue-100">Maximum height</text>
      </column>
    </column>
  </column>;
}
