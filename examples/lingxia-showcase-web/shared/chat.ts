// Shapes both halves of the page need. Logic and View are separate TypeScript
// projects, so a type the View imports cannot live in the Logic file.

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
