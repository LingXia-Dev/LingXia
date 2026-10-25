export function render() {
  return <page testId="cover-page" className="bg-red-500">
    <image testId="cover-image" src="public/mark.png" alt="" className="absolute inset-0 object-cover" />
    <column className="grow items-center justify-center">
      <text testId="cover-label" className="text-white text-lg bg-black/50 px-3 py-1">Square mark, cover crop</text>
    </column>
  </page>;
}
