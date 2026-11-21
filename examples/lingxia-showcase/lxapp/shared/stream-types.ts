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

export interface SendMessage { text: string; chartKind?: string }
export interface StreamPageData { messages: Message[]; isStreaming: boolean }

export type ChatChunk =
  | { type: 'token'; token: string }
  | { type: 'artifact'; chart: ChartData };
