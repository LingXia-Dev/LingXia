// Wrapping, truncation, line clamps, preserved whitespace and breaks inside long words.
const lorem = 'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore.';

export function render() {
  return (
    <page className="bg-canvas p-4 gap-3">
      <text className="text-base leading-relaxed">{lorem}</text>
      <text className="text-sm truncate">{lorem}</text>
      <text className="text-sm line-clamp-2">{lorem.repeat(3)}</text>
      <text className="text-xs font-mono whitespace-pre-wrap p-2 rounded bg-surface-sunken">{'Line one\n    indented line two\nline three'}</text>
      <text className="text-sm break-all">https://example.com/a/very/long/path/without/any/spaces/to/break/at</text>
      <row className="gap-2 items-baseline">
        <text className="text-3xl font-bold tabular-nums">1,234</text>
        <text className="text-xs text-muted">units</text>
        <text className="min-w-0 shrink truncate text-sm">{lorem}</text>
      </row>
    </page>
  );
}
