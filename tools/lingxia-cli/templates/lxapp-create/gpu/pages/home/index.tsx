import { useAction, useLocal } from '@lingxia/view';
import type { View } from './$types';

export function render({ data, actions }: View) {
  const name = useLocal('name', '');
  const greet = useAction(actions.greet);
  const trimmed = name.value.trim();
  const submit = () => { if (trimmed) greet.run({ name: trimmed }); };

  return (
    <page className="p-6 gap-4">
      <text data-testid="home-title" className="text-2xl font-semibold">Hello, LingXia</text>
      <input
        data-testid="home-name"
        aria-label="Your name"
        placeholder="Enter your name"
        bind={name}
        onSubmit={submit}
        className="min-h-11 px-4 rounded-lg bg-surface-sunken"
      />
      <button data-testid="home-greet" onClick={submit} disabled={!trimmed || greet.pending} className="min-h-11 rounded-lg bg-accent items-center justify-center">
        <text className="text-on-accent font-medium">Say Hello</text>
      </button>
      {data.greeting ? <text data-testid="home-greeting">{data.greeting}</text> : null}
    </page>
  );
}
