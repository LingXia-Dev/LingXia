import { useLocal, useView } from '@lingxia/view';

export function render() {
  const { env } = useView();
  const value = useLocal('shadow-input', 'MMMM');
  const cleared = useLocal('shadow-cleared', false);
  return <view className="bg-white p-4 gap-3" style={{paddingTop:env.insets.top+16}}>
    <button data-testid="shadow-toggle" className="w-60 h-10 bg-blue-600 text-white" onClick={() => cleared.set(!cleared.value)}>Toggle shadow</button>
    <view className="grid *:col-start-1 *:row-start-1 w-70 h-24">
      <text data-testid="shadow-layers" className="w-30 h-12 rounded-xl bg-white/50 text-xs" style={{left:20,top:16,boxShadow:cleared.value?'none':[{offsetX:12,color:'#ff000080'},{offsetX:20,color:'#0000ff'}]}}>Layers</text>
    </view>
    <view className="grid *:col-start-1 *:row-start-1 w-70 h-24">
      <input data-testid="shadow-input" aria-label="Shadow input" bind={value} className="w-30 h-12 px-3 rounded-xl bg-white text-black shadow-[0_0_16px_#000000]" style={{left:20,top:16}} />
    </view>
    <text data-testid="shadow-input-value" className="text-sm">{value.value}</text>
    <view className="grid *:col-start-1 *:row-start-1 w-70 h-24">
      <text data-testid="shadow-tailwind" className="w-40 h-12 bg-white rounded-xl text-sm shadow-lg shadow-blue-500/40" style={{left:20,top:8}}>Tailwind shadow</text>
    </view>
  </view>;
}
