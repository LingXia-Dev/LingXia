// What a page knows about its screen: the environment values it lays out from, one readout each.
import { useView } from '@lingxia/view';
import { Button } from '../ui';
import { Readout, useSay } from './demo';
import { labels } from './text';

const round = (n: number) => String(Math.round(n * 10) / 10);
const edges = (box: { top: number; right: number; bottom: number; left: number }) =>
  ([['top', box.top], ['right', box.right], ['bottom', box.bottom], ['left', box.left]] as const);

/** The capsule as a readout value: `x,y,width,height`, or `none`. */
export function capsuleState(capsule: { x: number; y: number; width: number; height: number } | null): string {
  return capsule ? [capsule.x, capsule.y, capsule.width, capsule.height].map(round).join(',') : 'none';
}

export function ScreenFacts() {
  const { env } = useView();
  const t = useSay();
  const row = 'flex-row gap-2 items-baseline';
  const name = 'w-24 text-xs text-muted';
  const capsule = capsuleState(env.capsule);
  return (
    <view className="gap-1">
      {([['safe', labels.envSafe, env.safeArea], ['insets', labels.envInsets, env.insets]] as const).map(([key, label, box]) => (
        <view key={key} className={row}>
          <text className={name}>{t(label)}</text>
          {edges(box).map(([edge, value]) => (
            <Readout key={edge} testId={`lab-env-${key}-${edge}`} state={round(value)}>{`${edge[0]} ${round(value)}`}</Readout>
          ))}
        </view>
      ))}
      <view className={row}>
        <text className={name}>{t(labels.envCapsule)}</text>
        <Readout testId="lab-env-capsule" state={capsule}>{capsule === 'none' ? t(labels.envNone) : capsule}</Readout>
      </view>
      <view className={row}>
        <text className={name}>{t(labels.envSize)}</text>
        <Readout testId="lab-env-size-class" state={env.sizeClass ?? 'compact'}>{env.sizeClass ?? 'compact'}</Readout>
      </view>
      <view className={row}>
        <text className={name}>{t(labels.envTextScale)}</text>
        <Readout testId="lab-env-text-scale" state={String(env.textScale)}>{`×${env.textScale}`}</Readout>
      </view>
      <view className={row}>
        <text className={name}>{t(labels.envContrast)}</text>
        <Readout testId="lab-env-contrast" state={env.highContrast ? 'high' : 'standard'}>{env.highContrast ? 'high' : 'standard'}</Readout>
      </view>
      <view className={row}>
        <text className={name}>{t(labels.envTheme)}</text>
        <Readout testId="lab-env-appearance" state={env.appearance}>{env.appearance}</Readout>
      </view>
      <view className={row}>
        <text className={name}>{t(labels.envMotion)}</text>
        <Readout testId="lab-env-reduced-motion" state={env.reducedMotion ? 'reduce' : 'full'}>{env.reducedMotion ? 'reduce' : 'full'}</Readout>
        <Readout testId="lab-env-duration-scale" state={round(env.animatorDurationScale)}>{`×${round(env.animatorDurationScale)}`}</Readout>
      </view>
      <view className={row}>
        <text className={name}>{t(labels.envViewport)}</text>
        <Readout testId="lab-env-viewport" state={`${round(env.viewport.width)},${round(env.viewport.height)}`}>{`${round(env.viewport.width)} × ${round(env.viewport.height)}`}</Readout>
      </view>
      <view className={row}>
        <text className={name}>{t(labels.envKeyboard)}</text>
        <Readout testId="lab-env-keyboard" state={round(env.keyboard.height)}>{round(env.keyboard.height)}</Readout>
      </view>
      <view className={row}>
        <text className={name}>{t(labels.envDevice)}</text>
        <Readout testId="lab-env-os" state={env.os}>{env.os}</Readout>
        <Readout testId="lab-env-pointer" state={env.pointer}>{env.pointer}</Readout>
      </view>
      <view className={row}>
        <text className={name}>{t(labels.envLanguage)}</text>
        <Readout testId="lab-env-language" state={env.language}>{env.language}</Readout>
        <Readout testId="lab-env-direction" state={env.direction}>{env.direction}</Readout>
        <Readout testId="lab-env-hour-cycle" state={env.hourCycle}>{env.hourCycle}</Readout>
      </view>
      <view className={row}>
        <text className={name}>{t(labels.envRenderer)}</text>
        <Readout testId="lab-env-renderer" state={env.renderer?.fallback ?? 'gpu'}>{env.renderer?.fallback ? `cpu (${env.renderer.fallback})` : 'gpu'}</Readout>
      </view>
    </view>
  );
}

export function ScreenDemo() {
  const t = useSay();
  return (
    <>
      <ScreenFacts />
      <view className="flex-row">
        <Button testId="lab-env-open" size="sm" variant="secondary" to={{ page: 'view-screen' }}>{t(labels.envOpen)}</Button>
      </view>
    </>
  );
}
