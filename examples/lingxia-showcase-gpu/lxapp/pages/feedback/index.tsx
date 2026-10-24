import { useLocal, type View } from '@lingxia/ui';
import '../../theme/index.css';

export function render({ actions, env }: View) {
  const category = useLocal('category', 'Product');
  const message = useLocal('message', '');
  const email = useLocal('email', '');
  const submitting = useLocal('submitting', false);
  const error = useLocal('error', '');
  const canSubmit = Boolean(message.value.trim()) && !submitting.value;
  const height = Math.max(1, env.viewport.height - env.insets.top - Math.max(env.insets.bottom, env.keyboard.height));
  const submit = async () => {
    if (!message.value.trim() || submitting.value) return;
    submitting.set(true); error.set('');
    try {
      await actions.submitFeedback({ category: category.value, message: message.value, email: email.value });
    } catch (cause) {
      error.set(cause instanceof Error ? cause.message : String(cause));
    } finally {
      submitting.set(false);
    }
  };
  return <column className="bg-surface-50 text-gray-900">
    <spacer style={{ height: env.insets.top }} />
    <row style={{ height }}><spacer style={{ width: env.insets.left }} />
      <scroll testId="page-scroll" className="grow basis-0 min-w-0" style={{ height }}>
        <column className="px-5 py-6 items-center">
          <column className="w-full max-w-lg gap-6">
            <column>
              <column className="w-11 h-11 items-center justify-center rounded-2xl bg-emerald-100"><vector className="w-5 h-5 text-gray-900" paths={[{ d: "M12 1C13.5 8.5 15.5 10.5 23 12C15.5 13.5 13.5 15.5 12 23C10.5 15.5 8.5 13.5 1 12C8.5 10.5 10.5 8.5 12 1Z", fill: "currentColor" }]} /></column><spacer className="h-3" />
              <text className="text-2xl font-semibold">Help us improve</text>
              <text className="pt-2 text-sm leading-6 text-gray-500">Share a bug, an idea, or anything that felt confusing in the showcase.</text>
            </column>
            <column className="gap-3">
              <text className="text-sm font-medium text-gray-700">What is this about?</text>
              <row className="gap-2">
                {['Product', 'Bug', 'Idea'].map(item => <button key={item} testId={`feedback-${item.toLowerCase()}`} enabled={!submitting.value} onClick={() => category.set(item)} className={category.value === item ? 'grow basis-0 w-0 h-10 rounded-xl border border-emerald-500 bg-emerald-50 text-sm font-medium text-emerald-700 dark:text-emerald-400' : 'grow basis-0 w-0 h-10 rounded-xl border border-line-200 bg-surface text-sm font-medium text-gray-600'}>{item}</button>)}
              </row>
            </column>
            <column className="gap-2">
              <text className="text-sm font-medium text-gray-700">Your feedback</text>
              <textarea testId="feedback-message" bind={message} enabled={!submitting.value} placeholder="What happened, and what would you prefer?" className="h-44 rounded-2xl border border-line-200 bg-surface px-4 py-3 text-sm leading-6" />
            </column>
            <column className="gap-2">
              <text className="text-sm font-medium text-gray-700">Email (optional)</text>
              <input testId="feedback-email" bind={email} enabled={!submitting.value} keyboard="email" placeholder="you@example.com" className="h-11 rounded-xl border border-line-200 bg-surface px-4 text-sm" />
            </column>
            <button testId="feedback-submit" enabled={canSubmit} onClick={submit} className={canSubmit ? 'h-12 rounded-2xl bg-surface-900 text-sm font-semibold text-white' : 'h-12 rounded-2xl bg-surface-900 text-sm font-semibold text-white opacity-35'}>{submitting.value ? 'Sending…' : 'Send feedback'}</button>
            {error.value ? <text testId="feedback-error" className="text-sm text-red-500">{error.value}</text> : null}
            <text className="text-center text-xs text-gray-400">Demo only — submissions are written to the Logic log.</text>
          </column>
        </column>
      </scroll><spacer style={{ width: env.insets.right }} />
    </row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} />
  </column>;
}
