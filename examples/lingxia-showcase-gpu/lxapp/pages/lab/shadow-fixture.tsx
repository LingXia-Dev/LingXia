import { useLocal, type Env } from '@lingxia/ui';
import '../../theme/index.css';

export function shadowFixture(env: Env) {
  const value = useLocal('shadow-input', 'MMMM');
  const cleared = useLocal('shadow-cleared', false);
  return <column className="bg-white p-4 gap-3" style={{paddingTop:env.insets.top+16}}>
    <button testId="shadow-toggle" className="w-60 h-10 bg-blue-600 text-white" onClick={() => cleared.set(!cleared.value)}>Toggle shadow</button>
    <stack className="w-70 h-24">
      <text testId="shadow-layers" className="w-30 h-12 rounded-xl bg-white/50 text-xs" style={{offset:{x:20,y:16},boxShadow:cleared.value?'none':[{offsetX:12,color:'#ff000080'},{offsetX:20,color:'#0000ff'}]}}>Layers</text>
    </stack>
    <stack className="w-70 h-24">
      <input testId="shadow-input" bind={value} className="w-30 h-12 px-3 rounded-xl bg-white text-black shadow-[0_0_16px_#000000]" style={{offset:{x:20,y:16}}} />
    </stack>
    <text testId="shadow-input-value" className="text-sm">{value.value}</text>
    <stack className="w-70 h-24">
      <text testId="shadow-tailwind" className="w-40 h-12 bg-white rounded-xl text-sm shadow-lg shadow-blue-500/40" style={{offset:{x:20,y:8}}}>Tailwind shadow</text>
    </stack>
  </column>;
}
