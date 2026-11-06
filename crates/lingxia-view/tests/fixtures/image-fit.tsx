// One landscape image in wide and tall boxes: cover, contain and fill, round and square.
export function render() {
  const fits = ['object-cover', 'object-contain', 'object-fill'];
  return (
    <page className="bg-canvas p-4 gap-3">
      <grid className="grid-cols-2 gap-3">
        {fits.map(fit => <image key={fit} src="image.png" alt="" className={`h-20 w-full rounded-xl bg-surface-sunken ${fit}`} />)}
      </grid>
      <row className="gap-3">
        {fits.map(fit => <image key={fit} src="image.png" alt="" className={`grow h-32 rounded-lg bg-surface-sunken ${fit}`} />)}
      </row>
      <image src="image.png" alt="" className="w-16 h-16 rounded-full object-cover" />
    </page>
  );
}
