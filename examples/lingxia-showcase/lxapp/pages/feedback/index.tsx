import React from 'react';
import { useLxPage } from '@lingxia/react';
import '../../tailwind.css';
import './feedback.css';

type FeedbackCategory = 'Product' | 'Bug' | 'Idea';

type PageData = Record<string, never>;

type PageActions = {
  submitFeedback: (params: {
    category: FeedbackCategory;
    message: string;
    email: string;
  }) => Promise<void>;
};

const categories: FeedbackCategory[] = ['Product', 'Bug', 'Idea'];

export default function FeedbackPage() {
  const { actions } = useLxPage<PageData, PageActions>();
  const [category, setCategory] = React.useState<FeedbackCategory>('Product');
  const [message, setMessage] = React.useState('');
  const [email, setEmail] = React.useState('');
  const [submitting, setSubmitting] = React.useState(false);

  const submit = React.useCallback(async () => {
    if (!message.trim() || submitting) return;
    setSubmitting(true);
    try {
      await actions.submitFeedback({ category, message, email });
    } finally {
      setSubmitting(false);
    }
  }, [actions, category, email, message, submitting]);

  return (
    <main className="min-h-screen bg-gray-50 px-5 py-6 text-gray-900">
      <div className="mx-auto flex w-full max-w-lg flex-col gap-6">
        <header>
          <div className="mb-3 flex h-11 w-11 items-center justify-center rounded-2xl bg-emerald-100 text-xl">
            ✦
          </div>
          <h1 className="text-2xl font-semibold tracking-tight">Help us improve</h1>
          <p className="mt-2 text-sm leading-6 text-gray-500">
            Share a bug, an idea, or anything that felt confusing in the showcase.
          </p>
        </header>

        <section className="space-y-3">
          <div className="text-sm font-medium text-gray-700">What is this about?</div>
          <div className="grid grid-cols-3 gap-2">
            {categories.map((item) => (
              <button
                key={item}
                type="button"
                onClick={() => setCategory(item)}
                className={`h-10 rounded-xl border text-sm font-medium transition-colors ${
                  category === item
                    ? 'border-emerald-500 bg-emerald-50 text-emerald-700'
                    : 'border-gray-200 bg-white text-gray-600'
                }`}
              >
                {item}
              </button>
            ))}
          </div>
        </section>

        <label className="space-y-2">
          <span className="text-sm font-medium text-gray-700">Your feedback</span>
          <textarea
            value={message}
            onChange={(event) => setMessage(event.target.value)}
            placeholder="What happened, and what would you prefer?"
            rows={6}
            className="w-full resize-none rounded-2xl border border-gray-200 bg-white px-4 py-3 text-sm leading-6 outline-none transition focus:border-emerald-500 focus:ring-2 focus:ring-emerald-100"
          />
        </label>

        <label className="space-y-2">
          <span className="text-sm font-medium text-gray-700">Email (optional)</span>
          <input
            type="email"
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            placeholder="you@example.com"
            className="h-11 w-full rounded-xl border border-gray-200 bg-white px-4 text-sm outline-none transition focus:border-emerald-500 focus:ring-2 focus:ring-emerald-100"
          />
        </label>

        <button
          type="button"
          disabled={!message.trim() || submitting}
          onClick={submit}
          className="h-12 rounded-2xl bg-gray-900 text-sm font-semibold text-white transition disabled:cursor-not-allowed disabled:opacity-35"
        >
          {submitting ? 'Sending…' : 'Send feedback'}
        </button>

        <p className="text-center text-xs text-gray-400">
          Demo only — submissions are written to the Logic log.
        </p>
      </div>
    </main>
  );
}
