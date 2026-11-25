import { createJinshuTools } from '../source/index.ts';

/** An `AgentCall` as a host would fill it in. */
export function agentCall(overrides = {}) {
  return {
    caller: { appId: 'dazi', name: 'Dazi' },
    conversation: 'conv-1',
    agent: 'Codex',
    approved: true,
    language: 'en-US',
    signal: new AbortController().signal,
    ...overrides,
  };
}

/** Tools over fixed sources, called the way the host calls them: assess, then run. */
export function toolsOver(sources, options = {}) {
  const tools = createJinshuTools({
    documents: {
      list: query => Object.entries(sources)
        .filter(([, s]) => (s.getState().doc.title ?? '').includes(query))
        .map(([key, s]) => ({ key, title: s.getState().doc.title ?? '' })),
      open: key => sources[key] ?? null,
      peek: key => sources[key],
      pageOf: key => `pages/doc?key=${key}`,
    },
    ...options,
  });
  return {
    tools,
    async assess(name, args, call = agentCall()) {
      const tool = tools.find(t => t.name === name);
      return typeof tool.effect === 'string' ? { effect: tool.effect } : tool.effect(args, call);
    },
    call(name, args, call = agentCall()) {
      return tools.find(t => t.name === name).run(args, call);
    },
  };
}
