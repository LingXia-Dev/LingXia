export interface ChartData {
  kind: 'bar' | 'line' | 'pie';
  title: string;
  series: { label: string; value: number }[];
}

export interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  chart?: ChartData;
}

export type ChatChunk =
  | { type: 'token'; token: string }
  | { type: 'artifact'; chart: ChartData };

interface MockScenario {
  text: string;
  chart?: ChartData;
}

const SCENARIOS: MockScenario[] = [
  {
    text:
      'LingXia bridge uses a bidirectional v2 protocol between the native host ' +
      'and the WebView. Each async generator yield becomes an event frame \u2014 ' +
      'no full-state diff, just a delta token. ' +
      'The View calls cancel() to trigger generator .return(), ' +
      'which fires the finally block for cleanup.',
  },
  {
    text:
      'LingXia streams wrap async generators in the page logic layer. ' +
      'LxStream emits "data" events for each yielded chunk, ' +
      '"end" when the generator returns, ' +
      'and "error" if an exception propagates. ' +
      'Sequence numbers guarantee ordering even under load.',
  },
  {
    text: "Here's the revenue breakdown for H1 2024:",
    chart: {
      kind: 'bar',
      title: 'Monthly Revenue (\u00A5K)',
      series: [
        { label: 'Jan', value: 42 },
        { label: 'Feb', value: 58 },
        { label: 'Mar', value: 39 },
        { label: 'Apr', value: 67 },
        { label: 'May', value: 72 },
        { label: 'Jun', value: 54 },
      ],
    },
  },
  {
    text: 'Performance benchmark across bridge transport modes:',
    chart: {
      kind: 'bar',
      title: 'Throughput (msg/s)',
      series: [
        { label: 'MessagePort', value: 4800 },
        { label: 'WebKit',      value: 3200 },
        { label: 'JSInterface', value: 2900 },
      ],
    },
  },
  {
    text: 'User growth has been accelerating each quarter:',
    chart: {
      kind: 'line',
      title: 'DAU Growth (thousands)',
      series: [
        { label: 'Q1 \'24', value: 12 },
        { label: 'Q2 \'24', value: 28 },
        { label: 'Q3 \'24', value: 41 },
        { label: 'Q4 \'24', value: 69 },
        { label: 'Q1 \'25', value: 95 },
      ],
    },
  },
  {
    text: 'Bridge latency over the past 6 months (p95, ms):',
    chart: {
      kind: 'line',
      title: 'p95 Latency (ms)',
      series: [
        { label: 'Nov', value: 18 },
        { label: 'Dec', value: 22 },
        { label: 'Jan', value: 15 },
        { label: 'Feb', value: 12 },
        { label: 'Mar', value: 10 },
        { label: 'Apr', value: 9  },
      ],
    },
  },
  {
    text: 'Traffic is distributed across these sources:',
    chart: {
      kind: 'pie',
      title: 'Traffic Sources',
      series: [
        { label: 'Organic',  value: 38 },
        { label: 'Referral', value: 27 },
        { label: 'Social',   value: 21 },
        { label: 'Direct',   value: 14 },
      ],
    },
  },
  {
    text: 'Platform distribution of active users:',
    chart: {
      kind: 'pie',
      title: 'Platform Share',
      series: [
        { label: 'iOS',       value: 41 },
        { label: 'Android',   value: 35 },
        { label: 'HarmonyOS', value: 16 },
        { label: 'macOS',     value: 8  },
      ],
    },
  },
];

async function* mockChatStream(): AsyncGenerator<ChatChunk, void> {
  const scenario = SCENARIOS[Math.floor(Math.random() * SCENARIOS.length)];

  await new Promise<void>((r) => setTimeout(r, 350 + Math.random() * 450));

  for (const word of scenario.text.split(' ')) {
    await new Promise<void>((r) => setTimeout(r, 35 + Math.random() * 55));
    yield { type: 'token', token: word + ' ' };
  }

  if (scenario.chart) {
    await new Promise<void>((r) => setTimeout(r, 180));
    yield { type: 'artifact', chart: scenario.chart };
  }
}

Page({
  data: {
    messages: [] as Message[],
    isStreaming: false,
  },

  async *onSend(params: { text: string }): AsyncGenerator<ChatChunk, void> {
    const text = (params?.text ?? '').trim();
    if (!text || this.data.isStreaming) return;

    const userMsg: Message = {
      id: `u${Date.now()}`,
      role: 'user',
      content: text,
    };
    this.setData({
      messages: [...this.data.messages, userMsg],
      isStreaming: true,
    });

    let accumulated = '';
    let chartData: ChartData | undefined;

    try {
      for await (const chunk of mockChatStream()) {
        if (chunk.type === 'token') accumulated += chunk.token;
        if (chunk.type === 'artifact') chartData = chunk.chart;
        yield chunk;
      }
    } finally {
      const assistantMsg: Message = {
        id: `a${Date.now()}`,
        role: 'assistant',
        content: accumulated || '(no response)',
        chart: chartData,
      };
      this.setData({
        messages: [...this.data.messages, assistantMsg],
        isStreaming: false,
      });
    }
  },

  onClear() {
    if (this.data.isStreaming) return;
    this.setData({ messages: [] });
  },
});
