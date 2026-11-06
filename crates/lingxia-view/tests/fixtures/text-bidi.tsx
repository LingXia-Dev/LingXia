// Right-to-left runs, mixed-direction lines and logical insets; the `rtl` state flips the page.
export function render() {
  return (
    <page className="bg-canvas p-4 gap-3">
      <text role="heading" className="text-xl font-semibold text-start">הגדרות</text>
      <text dir="rtl" className="text-sm">مرحبا بالعالم، هذا سطر طويل يلتف عبر عدة أسطر في الحاوية.</text>
      <text className="text-sm">Item 1 is called שלום עולם (version 2.1) in Hebrew.</text>
      <text dir="rtl" className="text-sm">הפריט Item-1 עולה 42.50 ₪ לחודש.</text>
      {['ltr', 'rtl', 'auto'].map(dir => (
        <row key={dir} dir={dir as 'ltr' | 'rtl' | 'auto'} className="ps-4 pe-2 py-2 gap-2 items-center rounded-s-xl border-s-4 border-accent bg-surface">
          <text className="grow text-sm text-start">{dir === 'ltr' ? 'Item 1' : 'פריט 1'}</text>
          <vector aria-hidden className="w-4 h-4 text-muted rtl:-scale-x-100" paths={[{ d: 'M9 18l6-6-6-6', fill: 'none', stroke: 'currentColor', strokeWidth: 1.5 }]} />
        </row>
      ))}
    </page>
  );
}
