import { useLocal, type View } from '@lingxia/ui';

const field = 'px-3 py-2.5 rounded-xl border border-line-strong bg-surface text-sm focus:border-focus';

export function render({ data }: View<object, { plans: { value: string; label: string; disabled?: boolean }[] }>) {
  const email = useLocal('email', '');
  const phone = useLocal('phone', '');
  const time = useLocal('time', '');
  const plan = useLocal('plan', '');
  const note = useLocal('note', '');
  const query = useLocal('query', '');
  return (
    <page className="bg-canvas">
      <scroll className="grow">
        <column className="p-4 gap-3">
          <text role="heading" className="text-xl font-semibold">Account</text>
          <search bind={query} aria-label="Search settings" placeholder="Search settings" className={field} />
          <input bind={email} aria-label="Email" type="email" autoComplete="email" autoCapitalize="none" spellCheck={false} enterKeyHint="next" placeholder="you@example.com" className={field} />
          <input bind={phone} aria-label="Phone" type="tel" autoComplete="tel" maxLength={16} enterKeyHint="next" placeholder="+1 555 0100" className={field} />
          <row className="gap-2">
            <input bind={time} aria-label="Quiet hours" type="time" placeholder="22:00" className={`grow basis-0 ${field}`} />
            <select bind={plan} aria-label="Plan" placeholder="Choose a plan" options={data.plans} className={`grow basis-0 ${field}`} />
          </row>
          <textarea bind={note} rows={3} maxRows={6} aria-label="Notes" placeholder="Anything we should know?" className={field} />
          <input disabled aria-label="Account id" value="HT-0042-77" className={field} />
          <text className="text-xs text-danger">Email is required.</text>
          <button onClick={() => {}} className="py-3 rounded-xl bg-accent text-on-accent active:opacity-80 disabled:opacity-40" disabled={!email.value}>Continue</button>
        </column>
      </scroll>
    </page>
  );
}
