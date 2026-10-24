import { useLocal, type View } from '@lingxia/ui';
import '../../theme/index.css';

export function render({ data, actions, env }: View) {
  const count = useLocal('counter', 0);
  const message = useLocal('message', '');
  const height = Math.max(1, env.viewport.height - env.insets.top - Math.max(env.insets.bottom, env.keyboard.height));
  const card = 'bg-surface rounded-xl border border-line-200 p-4 gap-3 shadow-sm';
  const label = 'text-xs text-gray-500';
  return <column testId="surface-page" className="bg-surface-100 text-gray-900">
    <spacer style={{ height: env.insets.top }} />
    <row style={{ height }}><spacer style={{ width: env.insets.left }} />
      <scroll testId="surface-scroll" className="grow basis-0 min-w-0" style={{ height }}>
        <column className="px-4 py-6 items-center"><column className="w-full max-w-md gap-6">
          <column className="gap-1"><text className="text-lg font-semibold">Surface Page</text><text className="text-sm text-gray-500">Inspect the query string and send a message to the opener.</text></column>
          <column className="bg-surface rounded-xl border border-line-200 p-4 gap-2 shadow-sm"><text className={label}>QUERY STRING</text><text testId="surface-query" className="font-mono text-sm text-gray-800">{String(data.queryString || '(none)')}</text></column>
          <column className={card}>
            <text className={label}>PAGE LIFECYCLE</text>
            <grid columns={2} className="gap-2">
              <column className="bg-surface-50 rounded-md px-3 py-2"><text className={label}>onShow</text><text testId="surface-show-count" className="font-mono text-base">{String(data.showCount || 0)}</text></column>
              <column className="bg-surface-50 rounded-md px-3 py-2"><text className={label}>onHide</text><text testId="surface-hide-count" className="font-mono text-base">{String(data.hideCount || 0)}</text></column>
            </grid>
            <row className="gap-1"><text className={label}>Last event:</text><text testId="surface-lifecycle" className="font-mono text-xs text-gray-800">{String(data.lastLifecycle || 'onLoad')}</text></row>
          </column>
          <column className={card}>
            <text className={label}>IN-PAGE COUNTER</text><text testId="surface-counter" className="font-mono text-2xl">{String(count.value)}</text>
            <text className={label}>Hide preserves this counter; close resets it on re-open.</text>
            <button testId="surface-increment" onClick={() => count.set(count.value + 1)} className="h-10 text-sm font-medium rounded-md bg-surface-200 text-gray-900">Increment</button>
          </column>
          <column className="bg-surface rounded-xl border border-line-200 p-4 gap-2 shadow-sm">
            <text className={label}>FROM OPENER</text><row className="gap-1"><text className={label}>Received:</text><text testId="surface-inbound-count" className="font-mono text-xs text-gray-800">{String(data.inboundCount || 0)}</text></row>
            <text testId="surface-inbound" className="font-mono text-sm text-gray-800">{String(data.lastInbound || '(none)')}</text>
          </column>
          <column className={card}>
            <text className={label}>MESSAGE</text>
            <input testId="surface-message" bind={message} placeholder="Message to parent page" className="px-3 py-2 rounded-md bg-surface border border-line-300 text-sm" />
            <button testId="surface-send-message" enabled={Boolean(message.value.trim())} onClick={async () => { await actions.logSurfaceMessage({ message: message.value.trim() }); message.set(''); }} className="h-10 text-sm font-medium rounded-md bg-blue-500 text-white">Send then close</button>
          </column>
          <column className={card}>
            <text className={label}>SELF ACTIONS</text>
            <button testId="surface-hide" onClick={() => actions.hideSelf()} className="h-10 text-sm font-medium rounded-md bg-amber-500 text-white">Hide (parent can show again)</button>
            <button testId="surface-close" onClick={() => actions.closeSelf()} className="h-10 text-sm font-medium rounded-md bg-rose-500 text-white">Close (destroys this page)</button>
          </column>
        </column></column>
      </scroll><spacer style={{ width: env.insets.right }} />
    </row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} />
  </column>;
}
