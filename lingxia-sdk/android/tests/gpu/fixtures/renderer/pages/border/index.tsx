import { useLocal, useView } from '@lingxia/view';

export function render() {
  const { env } = useView();
  const value = useLocal('border-input', 'MMMM');
  const range = useLocal('border-range', 0);
  const selection = useLocal('border-selection', -1);
  const clicks = useLocal('border-clicks', 0);
  return <view className="bg-white" style={{ paddingTop: env.insets.top, paddingBottom: env.insets.bottom }}>
    <view className="p-3 gap-2">
      <button data-testid="border-ring" onClick={() => clicks.set(clicks.value + 1)} className="w-40 h-16 rounded-3xl border-8 border-[#ff0000]/50 bg-white text-black">Ring</button>
      <scroll data-testid="border-scroll" className="w-40 h-20 rounded-[20px] border-t-2 border-r-4 border-b-[6px] border-l-8 border-[#ff0000] bg-white">
        <button data-testid="border-scroll-blue" onClick={() => clicks.set(clicks.value + 1)} className="h-40 rounded-none bg-[#0000ff] text-white">Blue</button>
        <button data-testid="border-scroll-green" onClick={() => clicks.set(clicks.value + 1)} className="h-40 rounded-none bg-[#00ff00] text-black">Green</button>
      </scroll>
      <input data-testid="border-input" aria-label="Border input" bind={value} className="w-70 h-14 p-2 rounded-none border-[6px] border-[#ff0000] bg-white text-black" />
      <slider data-testid="border-slider" aria-label="Border slider" bind={range} min={0} max={10} step={1} className="w-70 h-16 rounded-none border-l-[20px] border-r-[40px] border-t-2 border-b-[6px] border-current text-[#ff0000] bg-white" />
      <plot data-testid="border-plot" aria-label="Border plot" kind="bar" values={[3, 5, 4]} colors={['#0000ff']} selectedIndex={selection.value} onSelect={index => selection.set(index)} className="w-70 h-20 rounded-[20px] border-l-8 border-r-3 border-t-2 border-b-[6px] border-[#ff0000] bg-white" />
      <image data-testid="border-image" alt="App icon" src="public/AppIcon.png" className="object-fill w-16 h-16 rounded-[20px] border-8 border-[#ff0000] bg-white" />
      <text data-testid="border-status" className="text-xs text-black">{`${clicks.value}/${range.value}/${selection.value}`}</text>
    </view>
  </view>;
}
