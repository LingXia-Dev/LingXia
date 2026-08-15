import React from 'react';

export interface PageStackEntry {
  index: number;
  name: string;
  current: boolean;
}

/**
 * The one page-stack visualization used everywhere a demo shows the stack:
 * entries top-first, the current page marked, and a depth badge.
 */
export function PageStackCard({
  stack,
  badge,
  testId = 'page-stack',
}: {
  stack: PageStackEntry[];
  badge: string;
  testId?: string;
}) {
  return (
    <div className="bg-surface rounded-xl shadow-sm border border-line-100 overflow-hidden">
      <div className="px-4 py-3 border-b border-line-100 flex items-center justify-between">
        <div>
          <h3 className="text-base font-medium text-gray-900">Page stack</h3>
          <p className="text-xs text-gray-500 mt-0.5">getCurrentPages(), top is where you are</p>
        </div>
        <span
          data-testid={`${testId}-depth`}
          className="text-xs font-mono font-medium text-violet-600 dark:text-violet-400 bg-violet-50 dark:bg-violet-950 rounded-full px-2.5 py-1"
        >
          {badge}
        </span>
      </div>
      <div className="p-3 space-y-1.5" data-testid={`${testId}-list`}>
        {[...stack].reverse().map(entry => (
          <div
            key={entry.index}
            className={`flex items-center gap-3 rounded-lg px-3 py-2 text-xs font-mono ${
              entry.current
                ? 'bg-violet-50 dark:bg-violet-950 text-violet-700 dark:text-violet-300 border border-violet-200 dark:border-violet-800'
                : 'bg-surface-50 text-gray-600 border border-line-100'
            }`}
          >
            <span className="w-5 text-right opacity-60">{entry.index + 1}</span>
            <span className="flex-1 truncate">{entry.name}</span>
            {entry.current && <span className="text-[10px] font-sans font-medium">you are here</span>}
          </div>
        ))}
        {stack.length === 0 && (
          <div className="text-xs text-gray-400 text-center py-4">No page stack available</div>
        )}
      </div>
    </div>
  );
}
