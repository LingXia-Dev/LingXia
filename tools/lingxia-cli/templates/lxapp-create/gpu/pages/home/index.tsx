import { useAction, useLocal, useStream } from '@lingxia/view';
import type { View } from './$types';

export function render({ data, actions }: View) {
  const name = useLocal('name', '');
  const greeting = useLocal('greeting', '');
  const greet = useStream('greet', actions.greet, {
    onData: word => greeting.set(greeting.value ? `${greeting.value} ${word}` : word),
  });
  const reset = useAction(actions.reset);
  const trimmed = name.value.trim();
  const streaming = greet.status === 'streaming';
  const submit = () => {
    if (!trimmed || streaming) return;
    greeting.set('');
    greet.start({ name: trimmed });
  };

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
      <button
        data-testid="home-greet"
        onClick={submit}
        disabled={!trimmed || streaming}
        className="min-h-11 rounded-lg bg-accent items-center justify-center"
      >
        <text className="text-on-accent font-medium">Say Hello</text>
      </button>
      {greeting.value ? <text data-testid="home-greeting">{greeting.value}</text> : null}
      {data.greetCount > 0 ? (
        <button data-testid="home-reset" onClick={() => reset.run()} disabled={reset.pending} className="min-h-11 items-center justify-center">
          <text className="text-muted">Greeted {data.greetCount} times · Reset</text>
        </button>
      ) : null}
    </page>
  );
}
