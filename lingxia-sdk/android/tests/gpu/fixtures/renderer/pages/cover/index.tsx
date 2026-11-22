export function render() {
  return <page data-testid="cover-page" className="bg-red-500">
    <image data-testid="cover-image" src="public/mark.png" alt="" className="absolute inset-0 object-cover" />
    <view className="grow items-center justify-center">
      <text data-testid="cover-label" className="text-white text-lg bg-black/50 px-3 py-1">Square mark, cover crop</text>
    </view>
  </page>;
}
