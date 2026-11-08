export type PerfScene = 'list' | 'data' | 'state' | 'animation' | 'fill' | 'text' | 'touch' | 'transition' | 'images';
/** One Logic-owned record of the state scene. */
export interface PerfRow { id: number; title: string; note: string; count: number; done: boolean }
export interface PerfData {
  /** The scene a run drives, or '' between runs. */
  running: PerfScene | '';
  /** Ticks since the run started, about one per display frame. */
  tick: number;
  /** Flips every 600 ms during an animation run. */
  phase: boolean;
  /** The state scene's records, empty otherwise. */
  rows: PerfRow[];
  /** The bridge benchmark's write payload; null otherwise. */
  record?: Record<string, string | number> | null;
}
export interface DriveRequest { scene: PerfScene; ms: number; count: number }
export interface PerfActions {
  /** Start driving `scene` for `ms`; returns at once. */
  drive(request: DriveRequest): Promise<void>;
  /** Answers at once with the current tick: an action reply racing a running scene's writes. */
  ping(): Promise<number>;
  /** The bridge micro-benchmark: awaited writes, `ui.*` requests, and the cost of one `setData` call. */
  bench(request: BenchRequest): Promise<BenchResult>;
}
export interface BenchRequest { writes: number; requests: number; calls: number }
/** `setDataUs`: Logic time of one unawaited `setData` of a 20-field record, µs. */
export interface BenchResult { setDataUs: number }
