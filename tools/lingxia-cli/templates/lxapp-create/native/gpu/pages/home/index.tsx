import type { View } from './$types';

export function render(_: View) {
  return (
    <page className="p-6 gap-4">
      <text data-testid="home-title" className="text-2xl font-semibold">Hello, LingXia</text>
      <text className="text-muted">This host runs without AppService, so the page has no Logic: no data and no actions.</text>
    </page>
  );
}
