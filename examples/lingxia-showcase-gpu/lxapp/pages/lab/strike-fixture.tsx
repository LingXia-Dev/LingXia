import { useLocal, type Env } from '@lingxia/ui';
import '../../theme/index.css';
export function strikeFixture(env: Env) {
  const strike = useLocal('strike-fixture', false);
  return <column className="bg-white text-black p-4 gap-4" style={{ paddingTop: env.insets.top + 16 }}>
    <button testId="strike-toggle" className="h-12 bg-blue-600 text-white" onClick={() => strike.set(!strike.value)}>Toggle decoration</button>
    <column className={strike.value ? 'line-through' : 'no-underline'}>
      <text testId="strike-lines" className="w-80 h-24 text-[32px] leading-[48px] font-mono no-underline">{'MM        MM\nMM        MM'}</text>
    </column>
    <scroll testId="strike-scroll" className="w-80 h-24 bg-white">
      <spacer className="h-16 shrink-0" />
      <text className={strike.value ? 'w-80 h-24 shrink-0 text-[32px] leading-[48px] font-mono line-through' : 'w-80 h-24 shrink-0 text-[32px] leading-[48px] font-mono no-underline'}>{'MM        MM\nMM        MM'}</text>
    </scroll>
  </column>;
}
