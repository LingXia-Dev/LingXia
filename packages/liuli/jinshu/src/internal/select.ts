/** How many selectable text nodes a block takes from the page budget. */
export function selectCost(block: unknown): number {
  if (Array.isArray(block)) return block.reduce((sum: number, item) => sum + selectCost(item), 0);
  if (!block || typeof block !== 'object') return 0;
  const node = block as { readonly type?: unknown };
  let sum = node.type === 'paragraph' || node.type === 'heading' || node.type === 'code' ? 1 : 0;
  for (const value of Object.values(block)) if (value && typeof value === 'object') sum += selectCost(value);
  return sum;
}
