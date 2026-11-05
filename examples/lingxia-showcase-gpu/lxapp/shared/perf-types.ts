export type PerfScene = 'list' | 'data' | 'animation' | 'fill' | 'text' | 'touch' | 'transition' | 'images';
export interface PerfData {
  /** The scene a run drives, or '' between runs. */
  running: PerfScene | '';
  /** Ticks since the run started, about one per display frame. */
  tick: number;
  /** Flips every 600 ms during an animation run. */
  phase: boolean;
}
export interface DriveRequest { scene: PerfScene; ms: number; count: number }
export interface PerfActions {
  /** Start driving `scene` for `ms`; returns at once. */
  drive(request: DriveRequest): Promise<void>;
}
