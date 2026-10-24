import { type View } from '@lingxia/ui';
import '../../theme/index.css';

export function render({ data, actions, env }: View) {
  const refreshing = Boolean(data.isRefreshing);
  const height = Math.max(1, env.viewport.height - env.insets.top - Math.max(env.insets.bottom, env.keyboard.height));
  const card = 'bg-surface rounded-xl shadow-sm border border-line-200';
  return <column testId="pull-refresh-page" className="bg-surface-100 text-gray-900">
    <spacer style={{ height: env.insets.top }} />
    <row style={{ height }}><spacer style={{ width: env.insets.left }} />
      <scroll testId="page-scroll" className="grow basis-0 min-w-0" style={{ height }}>
        <column className="px-4 pt-6 pb-12 gap-4">
          <column className={card}>
            <column className="px-4 py-5 items-center text-center">
              <vector className="w-12 h-12 text-blue-500 dark:text-blue-400" paths={[{ d: 'M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15', fill: 'none', stroke: 'currentColor', strokeWidth: 1.5 }]} />
              <text testId="pull-refresh-title" className="text-base font-medium pt-3 pb-2">Pull Down Refresh Demo</text>
              <text className="text-sm text-gray-500">Pull down on this page to trigger refresh, or use the buttons below to control it programmatically.</text>
            </column>
          </column>
          <column className={card}>
            <column className="px-4 py-3 border-b border-line-100"><text className="text-base font-medium">Refresh Status</text></column>
            <column className="px-4 py-4 gap-3">
              <row className="items-center justify-between gap-3"><text className="text-sm text-gray-600">Status</text><text testId="pull-refresh-status" className={refreshing ? 'text-sm font-medium text-blue-600 dark:text-blue-400' : 'text-sm font-medium text-green-600 dark:text-green-400'}>{refreshing ? 'Refreshing...' : 'Idle'}</text></row>
              <row className="items-center justify-between gap-3"><text className="text-sm text-gray-600">Refresh Count</text><text testId="pull-refresh-count" className="text-sm font-medium">{String(data.refreshCount ?? 0)}</text></row>
              <row className="items-center justify-between gap-3"><text className="text-sm text-gray-600">Last Refresh</text><text testId="pull-refresh-time" className="text-sm font-medium">{String(data.lastRefreshTime || 'Never')}</text></row>
            </column>
          </column>
          <column className={card}>
            <column className="px-4 py-3 border-b border-line-100 gap-1"><text className="text-base font-medium">API Controls</text><text className="text-sm text-gray-500">Control refresh programmatically</text></column>
            <column className="p-4 gap-3">
              <button testId="pull-refresh-start" enabled={!refreshing} onClick={() => actions.startRefresh()} className={refreshing ? 'min-h-11 py-3 px-4 rounded-lg text-sm font-medium bg-surface-100 text-gray-400' : 'min-h-11 py-3 px-4 rounded-lg text-sm font-medium bg-blue-500 text-white'}>lx.startPullDownRefresh()</button>
              <button testId="pull-refresh-stop" onClick={() => actions.stopRefresh()} className="min-h-11 py-3 px-4 rounded-lg text-sm font-medium bg-surface-500 text-white">lx.stopPullDownRefresh()</button>
            </column>
          </column>
        </column>
      </scroll><spacer style={{ width: env.insets.right }} />
    </row><spacer style={{ height: Math.max(env.insets.bottom, env.keyboard.height) }} />
  </column>;
}
