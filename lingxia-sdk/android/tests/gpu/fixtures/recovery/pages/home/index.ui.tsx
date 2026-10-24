import { useLocal } from '@lingxia/ui';

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
  return <column style={{ padding: 24, gap: 20, background: '#f0f4f8' }}>
    <text testId="recovery-count">{`Count ${count.value}`}</text>
    <input testId="recovery-draft" bind={draft} />
    <button testId="recovery-increment" onClick={() => count.set(count.value + 1)}>Save count</button>
    <button testId="recovery-trigger" onClick={() => {
      failNext = true;
      count.set(count.value + 1);
    }}>Fail one update</button>
    <button testId="recovery-timeout" onClick={() => {
      timeoutNext = true;
      count.set(count.value + 1);
    }}>Interrupt session</button>
  </column>;
}
