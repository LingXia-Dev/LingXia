import React from 'react';
import { LxNavigator } from '@lingxia/react';
import '../../tailwind.css';

interface Demo {
  page: string;
  title: string;
  detail: string;
}

const GROUPS: { title: string; demos: Demo[] }[] = [
  {
    title: 'Native components in a page',
    demos: [
      { page: 'swiper', title: 'Media Swiper', detail: 'LxMediaSwiper over picked images and videos' },
      { page: 'navigator', title: 'Navigator', detail: 'LxNavigator: push, replace, reset, float, URLs' },
    ],
  },
  {
    title: 'Media',
    demos: [
      { page: 'video', title: 'Video', detail: 'The WebView’s own <video> element' },
    ],
  },
  {
    title: 'Bridge',
    demos: [
      { page: 'chat', title: 'Chat', detail: 'A stream: an async generator action, cancellable' },
      { page: 'channel', title: 'Channel', detail: 'A two-way session: live ticks and commands' },
      { page: 'bridge-repro', title: 'Bridge under load', detail: 'Bootstrap, echo and a gap-free tick stream' },
    ],
  },
  {
    title: 'Page',
    demos: [
      { page: 'pullToRefresh', title: 'Pull to Refresh', detail: 'Native pull over a web page' },
    ],
  },
];

export default function HomePage() {
  return (
    <div className="min-h-screen bg-surface-100" data-testid="home-page">
      <div className="px-4 py-5 pb-12 space-y-5">
        <div>
          <div className="text-xl font-semibold text-gray-900">LingXia Showcase (WebView)</div>
          <div className="text-sm text-gray-500 mt-1">What a WebView page can show: native components, the bridge, page chrome.</div>
        </div>
        {GROUPS.map((group) => (
          <section key={group.title} className="space-y-2">
            <h2 className="px-1 text-xs font-semibold uppercase tracking-wider text-gray-500">{group.title}</h2>
            <div className="bg-surface rounded-xl shadow-sm border border-line-200 overflow-hidden divide-y divide-line-100">
              {group.demos.map((demo) => (
                <LxNavigator key={demo.page} to={{ page: demo.page }} className="block">
                  <div
                    data-testid={`home-${demo.page}`}
                    className="px-4 py-3.5 flex items-center justify-between active:bg-surface-50"
                  >
                    <div>
                      <div className="text-sm font-medium text-gray-900">{demo.title}</div>
                      <div className="text-xs text-gray-500">{demo.detail}</div>
                    </div>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-5 h-5 text-gray-400">
                      <path d="M9 18l6-6-6-6" />
                    </svg>
                  </div>
                </LxNavigator>
              ))}
            </div>
          </section>
        ))}
      </div>
    </div>
  );
}
