// Built-in keyframe animations and a transition, sampled at the time each state names.
export function render() {
  return (
    <page className="bg-canvas p-6 gap-6">
      <row className="gap-6 items-center">
        <view className="w-12 h-12 rounded-lg bg-accent animate-spin" />
        <view className="w-12 h-12 rounded-full bg-chart-2 animate-ping" />
        <view className="w-12 h-12 rounded-full bg-chart-3 animate-pulse" />
        <view className="w-12 h-12 rounded-full bg-chart-4 animate-bounce" />
      </row>
      <view className="h-12 rounded-xl bg-surface transition duration-500 starting:opacity-0 starting:translate-x-24" />
    </page>
  );
}
