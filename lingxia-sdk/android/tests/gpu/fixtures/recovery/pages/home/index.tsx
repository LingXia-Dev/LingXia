import { useLocal } from '@lingxia/view';

let failNext = false;
let timeoutNext = false;

export function render() {
  const count = useLocal('count', 0);
  const draft = useLocal('draft', 'Retained draft');
  if (timeoutNext) { while (true) {} }
  if (failNext) {
    failNext = false;
    throw new Error('Deliberate recoverable candidate failure');
  }
  return <view className="p-6 gap-5 bg-[#f0f4f8]">
    <text data-testid="recovery-count">{`Count ${count.value}`}</text>
    <input data-testid="recovery-draft" aria-label="Draft" bind={draft} />
    <button data-testid="recovery-increment" onClick={() => count.set(count.value + 1)}>Save count</button>
    <button data-testid="recovery-trigger" onClick={() => {
      failNext = true;
      count.set(count.value + 1);
    }}>Fail one update</button>
    <button data-testid="recovery-timeout" onClick={() => {
      timeoutNext = true;
      count.set(count.value + 1);
    }}>Interrupt session</button>
  </view>;
}
