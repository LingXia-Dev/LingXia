import { useLocal, useView } from '@lingxia/view';
export function render() {
  const { env } = useView();
  const strike = useLocal('strike-fixture', false);
  return <view className="bg-white text-black p-4 gap-4" style={{ paddingTop: env.insets.top + 16 }}>
    <button data-testid="strike-toggle" className="h-12 bg-blue-600 text-white" onClick={() => strike.set(!strike.value)}>Toggle decoration</button>
    <view className={strike.value ? 'line-through' : 'no-underline'}>
      <text data-testid="strike-lines" className="w-80 h-24 text-[32px] leading-[48px] font-mono no-underline whitespace-pre">{'MM        MM\nMM        MM'}</text>
    </view>
    <scroll data-testid="strike-scroll" className="w-80 h-24 bg-white">
      <view className="h-16 shrink-0" />
      <text className={strike.value ? 'w-80 h-24 shrink-0 text-[32px] leading-[48px] font-mono line-through whitespace-pre' : 'w-80 h-24 shrink-0 text-[32px] leading-[48px] font-mono no-underline whitespace-pre'}>{'MM        MM\nMM        MM'}</text>
    </scroll>
  </view>;
}
