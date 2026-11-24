import { useView } from '@lingxia/view';

/** The environment an adaptive part reads: the size class, the viewport and the hinge. Nothing to press. */
export function AdaptiveStatus() {
  const { env } = useView();
  return (
    <view className="p-3 border-b border-line bg-surface-sunken grid grid-cols-1 regular:grid-cols-2 gap-2">
      <text data-testid="adaptive-size-class" className="font-mono text-xs text-muted">
        {`sizeClass=${env.sizeClass ?? 'compact'} · viewport=${Math.round(env.viewport?.width ?? 0)}×${Math.round(env.viewport?.height ?? 0)}`}
      </text>
      <text data-testid="adaptive-hinge" className="font-mono text-xs text-muted">
        {env.hinge ? `hinge=${env.hinge.orientation}/${env.hinge.posture}` : 'hinge=none'}
      </text>
    </view>
  );
}
