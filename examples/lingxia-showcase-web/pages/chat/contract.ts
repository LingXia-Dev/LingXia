import type { LxStream } from '@lingxia/bridge';
import type { PageShape } from '../../shared/lib/page-shape';

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

export type ChatPage = PageShape<
  { messages: Message[]; isStreaming: boolean },
  {
    onSend(params: { text: string }): LxStream<ChatChunk, void>;
    onClear(): Promise<void>;
    onOpenAbout(): Promise<void>;
    onOpenTerminal(): Promise<void>;
    onOpenBrowser(): Promise<void>;
  }
>;
