import { Card } from '@liulidesign/core/Card';
import { Sparkline } from '../Sparkline/Sparkline';
import { Stat } from './Stat';

const revenue = [31, 34, 33, 38, 36, 41, 40, 44, 47, 45, 48];
const latency = [4.1, 3.9, 4.0, 3.6, 3.7, 3.4, 3.3, 3.2];
const errors = [12, 11, 14, 13, 17, 19, 18, 23];

/** The figure, how it changed in the app's words, and its last eleven weeks. */
export function Revenue() {
  return (
    <Stat
      label="Revenue"
      value="$48,250"
      delta="12.4%"
      deltaDirection="up"
      deltaLabel="Up 12.4% from last month"
      description="vs last month"
      chart={<Sparkline values={revenue} variant="area" isSmooth hasEndDot tone="positive" label="Revenue by week: up from 31k to 48k" />}
      data-testid="revenue"
    />
  );
}

export function Plain() {
  return <Stat label="Open orders" value="128" data-testid="orders" />;
}

/** Down is good news for a wait, and up is bad news for errors: the tone is set apart from the direction. */
export function Tones() {
  return (
    <view className="grid grid-cols-2 gap-x-4 gap-y-5">
      <Stat
        label="Response time"
        value="3.2 s"
        delta="0.9 s"
        deltaDirection="down"
        deltaTone="positive"
        deltaLabel="Down 0.9 seconds, an improvement"
        chart={<Sparkline values={latency} tone="positive" label="Response time by day: down from 4.1 to 3.2 seconds" />}
        data-testid="latency"
      />
      <Stat
        label="Errors"
        value="23"
        delta="5"
        deltaDirection="up"
        deltaTone="negative"
        deltaLabel="Up 5 from yesterday, worse"
        chart={<Sparkline values={errors} tone="negative" label="Errors by day: up from 12 to 23" />}
        data-testid="errors"
      />
    </view>
  );
}

export function Sizes() {
  return (
    <view className="grid grid-cols-2 items-end gap-x-4">
      <Stat label="Small" value="1,204" size="sm" delta="3.1%" deltaDirection="up" data-testid="small" />
      <Stat label="Medium" value="1,204" delta="3.1%" deltaDirection="up" data-testid="medium" />
    </view>
  );
}

export function StateUp() {
  return <Stat label="Sign-ups" value="1,204" delta="3.1%" deltaDirection="up" deltaLabel="Up 3.1%" data-testid="up" />;
}

export function StateDown() {
  return <Stat label="Sign-ups" value="1,131" delta="6.0%" deltaDirection="down" deltaLabel="Down 6.0%" data-testid="down" />;
}

export function StateFlat() {
  return <Stat label="Sign-ups" value="1,204" delta="0.0%" deltaLabel="No change" data-testid="flat" />;
}

export function StateLoading() {
  return <Stat label="Sign-ups" value="" delta="3.1%" isLoading data-testid="loading" />;
}

/** Four figures on a phone: two columns of cards, each figure with how it changed. */
export function Dashboard() {
  return (
    <view className="grid grid-cols-2 gap-3">
      <Card variant="sunken">
        <Stat label="Revenue" value="$48,250" size="sm" delta="12.4%" deltaDirection="up" deltaLabel="Up 12.4%" />
      </Card>
      <Card variant="sunken">
        <Stat label="Orders" value="1,204" size="sm" delta="3.1%" deltaDirection="up" deltaLabel="Up 3.1%" />
      </Card>
      <Card variant="sunken">
        <Stat label="Refunds" value="37" size="sm" delta="8" deltaDirection="up" deltaTone="negative" deltaLabel="Up 8, worse" />
      </Card>
      <Card variant="sunken">
        <Stat label="Avg. order" value="$40.07" size="sm" delta="0.0%" deltaLabel="No change" />
      </Card>
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return <Stat label="Revenue" value="$48k" size="sm" delta="12%" deltaDirection="up" />;
}
