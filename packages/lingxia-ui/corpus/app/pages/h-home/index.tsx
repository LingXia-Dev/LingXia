// h-app pages/home (views/view.tsx, components/experience-card.tsx, network-card.tsx), ported
// construct for construct: the pinned location bar, greeting, the experience card with its
// band, score and five categories, the network status card, tool tiles, the last speed test,
// and the Family and Sharing rows. Classes are h-app's; dates come formatted in the data.
import { useLocal, type View } from '@lingxia/ui';
import '../../theme/heights.css';
import { Button, Icon, StatusDot, cn } from '../../shared/ui';
import type { IconName } from '../../shared/icons';

type Band = { code: 'excellent' | 'good' | 'fair' | 'poor'; label: string; score: number | null };
type Data = {
  locationName: string;
  network: 'online' | 'issue' | 'offline' | 'unavailable';
  dateLabel: string;
  greeting: string;
  userName: string;
  experience: { band: Band; headline: string; since: string; categories: { name: 'rtc' | 'gaming' | 'streaming' | 'live' | 'web'; label: string; band: Band | null; affected: boolean }[] };
  onlineDevices: number;
  nodeCount: number;
  lastSpeed: { downMbps: number; upMbps: number | null; ago: string } | null;
  channelLinked: boolean;
  experienceOpen: boolean;
};

const t = {
  healthGood: 'Network online', healthBad: 'Network needs attention', devicesOnline: 'devices online', meshPoints: 'Wi-Fi points',
  askWhy: 'Ask AI why', download: 'Download', upload: 'Upload', lastSpeed: 'Last speed test', tileWifi: 'Wi-Fi', tileMap: 'Map',
  tileSpeedtest: 'Speed test', tileSystem: 'System', familyTitle: 'Family',
  familyBlurb: 'Internet off in one tap, schedules and study time for the kids.', sharingTitle: 'Sharing',
  sharingBlurb: 'Who can reach this home, and who you have invited.', expDataAsOf: 'Data as of {time}', expExpand: 'See why',
};

const DOT = { online: 'online', issue: 'degraded', offline: 'offline', unavailable: 'unknown' } as const;
const CARD = 'rounded-lg border border-border bg-card shadow-[var(--card-shadow)]';
const BAND_INK = { excellent: 'text-success-subtle-foreground', good: 'text-success-subtle-foreground', fair: 'text-warning-subtle-foreground', poor: 'text-destructive-subtle-foreground' };
const BAND_MARK = { excellent: 'bg-success-subtle text-success-subtle-foreground', good: 'bg-success-subtle text-success-subtle-foreground', fair: 'bg-warning-subtle text-warning-subtle-foreground', poor: 'bg-destructive-subtle text-destructive-subtle-foreground' };
const CATEGORY_ICON: Record<Data['experience']['categories'][number]['name'], IconName> = { rtc: 'Video', gaming: 'Gamepad2', streaming: 'Clapperboard', live: 'Radio', web: 'Globe' };
const needsAttention = (band: Band) => band.code === 'fair' || band.code === 'poor';

function ExperienceCard({ experience, initiallyOpen }: { experience: Data['experience']; initiallyOpen: boolean }) {
  const open = useLocal('experienceOpen', initiallyOpen);
  const { band } = experience;
  return (
    <column className="mt-section" data-testid="experience">
      <column className={cn(CARD, 'pb-4')} data-testid="experience-card" data-band={band.code} data-open={open.value}>
        <row className="mt-3 items-center gap-3 px-4">
          <view aria-hidden className={cn('grid size-11 shrink-0 place-items-center rounded-full', BAND_MARK[band.code])}>
            <Icon name={needsAttention(band) ? 'CircleAlert' : 'Check'} size={22} />
          </view>
          <column className="min-w-0 flex-1">
            <row className="items-baseline gap-2" data-testid="home-band">
              <text dir="auto" className={cn('text-section-title leading-tight', BAND_INK[band.code])}>{band.label}</text>
              {band.score != null ? <text className="text-body font-semibold tabular-nums text-muted-foreground" data-testid="home-score">{String(band.score)}</text> : null}
            </row>
            <text dir="auto" className="text-supporting text-muted-foreground" data-testid="experience-headline">{experience.headline}</text>
            <text className="mt-0.5 text-caption text-muted-foreground" data-testid="experience-data-time">{t.expDataAsOf.replace('{time}', experience.since)}</text>
          </column>
        </row>
        <button onClick={() => open.set(!open.value)} aria-expanded={open.value} data-testid="experience-toggle"
          className="focus-ring mx-4 mt-3 inline-flex flex-row items-center gap-1 self-start rounded bg-transparent p-0">
          <text className="text-supporting font-medium text-primary">{t.expExpand}</text>
          <Icon name="ChevronDown" size={16} className={cn('text-primary transition-transform', open.value && 'rotate-180')} />
        </button>
        {open.value ? (
          <column className="mt-4 gap-4 border-t border-border px-4 pt-4" data-testid="experience-detail">
            <column role="list" className="gap-2.5" data-testid="experience-categories">
              {experience.categories.map(category => (
                <row key={category.name} role="listitem" className="min-w-0 items-center gap-3" data-testid={`experience-category-${category.name}`}>
                  <view aria-hidden className={cn('grid size-8 shrink-0 place-items-center rounded-full', category.band ? BAND_MARK[category.band.code] : 'border border-dashed border-border text-muted-foreground')}>
                    <Icon name={CATEGORY_ICON[category.name]} size={16} />
                  </view>
                  <text dir="auto" className={cn('min-w-0 flex-1 text-supporting', category.affected ? 'font-semibold text-foreground' : 'text-foreground')}>{category.label}</text>
                  {category.band ? (
                    <text className={cn('shrink-0 text-supporting font-medium', BAND_INK[category.band.code])}>
                      {category.band.label}
                      {category.band.score != null ? <span className="tabular-nums text-muted-foreground">{` ${category.band.score}`}</span> : null}
                    </text>
                  ) : null}
                </row>
              ))}
            </column>
          </column>
        ) : null}
      </column>
    </column>
  );
}

function NetworkCard({ data }: { data: Data }) {
  const issue = data.network === 'issue';
  return (
    <column className={`mt-3 ${CARD} p-3`} data-testid="health-card" data-state={data.network}>
      <row className="items-center gap-3">
        <view className={`grid size-9 shrink-0 place-items-center rounded-full ${issue ? 'bg-warning-subtle text-warning-subtle-foreground' : 'bg-success-subtle text-success-subtle-foreground'}`}>
          <Icon name={issue ? 'TriangleAlert' : 'Check'} size={18} />
        </view>
        <column className="min-w-0 flex-1">
          <text className="text-supporting font-medium" data-testid="health-title">{issue ? t.healthBad : t.healthGood}</text>
          <text className="text-caption text-muted-foreground" data-testid="health-detail">{`${data.onlineDevices} ${t.devicesOnline} · ${data.nodeCount} ${t.meshPoints}`}</text>
        </column>
      </row>
      {issue ? (
        <Button className="mt-3 w-full" onClick={() => {}} data-testid="ask-why">
          <row className="items-center gap-2"><Icon name="Sparkles" size={16} /><text className="text-control">{t.askWhy}</text></row>
        </Button>
      ) : null}
    </column>
  );
}

function Tiles() {
  const tiles: { key: string; label: string; icon: IconName }[] = [
    { key: 'wifi', label: t.tileWifi, icon: 'Wifi' }, { key: 'map', label: t.tileMap, icon: 'Map' },
    { key: 'speedtest', label: t.tileSpeedtest, icon: 'Gauge' }, { key: 'system', label: t.tileSystem, icon: 'Settings2' },
  ];
  return (
    <grid className="mt-3 grid grid-cols-4 gap-2">
      {tiles.map(({ key, label, icon }) => (
        <button key={key} onClick={() => {}} data-testid={`tile-${key}`}
          className="focus-ring grid justify-items-center gap-1.5 rounded-lg border border-border bg-card px-1 py-3 shadow-[var(--card-shadow)] transition-colors hover:bg-accent-soft grid-cols-1">
          <Icon name={icon} size={22} className="text-primary" />
          <text className="text-caption">{label}</text>
        </button>
      ))}
    </grid>
  );
}

function LastSpeed({ lastSpeed }: { lastSpeed: Data['lastSpeed'] }) {
  const figure = (label: string, icon: IconName, tone: string, value: number | null | undefined) => (
    <column>
      <row className="items-center gap-1">
        <Icon name={icon} size={14} className={tone} />
        <text className="text-supporting text-muted-foreground">{label}</text>
      </row>
      <text className="text-section-title tabular-nums">{value == null ? '—' : String(value)}<span className="text-caption font-normal text-muted-foreground"> Mbps</span></text>
    </column>
  );
  return (
    <column className={`mt-3 ${CARD} p-4`} data-testid="last-speed" data-state={lastSpeed ? 'result' : 'none'}>
      <row className="items-center justify-between gap-2">
        <text className="text-caption font-medium uppercase tracking-wide text-muted-foreground">{t.lastSpeed}</text>
        {lastSpeed ? <text className="text-caption text-muted-foreground">{lastSpeed.ago}</text> : null}
      </row>
      <grid className="mt-2 grid grid-cols-2 gap-4">
        {figure(t.download, 'ArrowDown', 'text-primary', lastSpeed?.downMbps)}
        {figure(t.upload, 'ArrowUp', 'text-upload', lastSpeed?.upMbps)}
      </grid>
    </column>
  );
}

function NavRow({ 'data-testid': testId, title, blurb, mark }: { 'data-testid': string; title: string; blurb: string; mark: JSX.Element }) {
  return (
    <button onClick={() => {}} data-testid={testId}
      className="focus-ring mt-3 flex w-full flex-row items-center gap-3 rounded-lg border border-border bg-card p-4 text-start shadow-[var(--card-shadow)] transition-colors hover:bg-accent-soft">
      {mark}
      <column className="min-w-0 flex-1">
        <text className="block text-item-title">{title}</text>
        <text className="block text-supporting text-muted-foreground">{blurb}</text>
      </column>
      <Icon name="ChevronRight" size={18} className="rtl:-scale-x-100 shrink-0 text-muted-foreground" />
    </button>
  );
}

export function render({ data }: View<object, Data>) {
  return (
    <page className="density-compact bg-background text-body text-foreground">
      <scroll className="grow">
        <column className="mx-auto w-full max-w-md px-gutter pb-[calc(max(2rem,env(safe-area-inset-bottom))_+_var(--lx-page-chrome-bottom-inset,0px))]">
          <row className="sticky top-0 z-20 -mx-gutter bg-background px-gutter pb-2 pt-[max(3.25rem,env(safe-area-inset-top))]" data-testid="location-bar">
            <button onClick={() => {}} data-testid="location-chip"
              className="focus-ring -ms-1 inline-flex flex-row items-center gap-1.5 rounded-full bg-transparent px-2 py-1">
              <StatusDot status={DOT[data.network]} size={8} />
              <text className="text-supporting text-muted-foreground">{data.locationName}</text>
              <Icon name="ChevronDown" size={14} className="text-muted-foreground" />
            </button>
          </row>
          <column>
            <text className="mt-1 text-supporting text-muted-foreground">{data.dateLabel}</text>
            <text role="heading" headingLevel={1} className="text-page-title">{`${data.greeting}, ${data.userName}`}</text>
          </column>
          <ExperienceCard experience={data.experience} initiallyOpen={data.experienceOpen} />
          <NetworkCard data={data} />
          <Tiles />
          <LastSpeed lastSpeed={data.lastSpeed} />
          <NavRow data-testid="family-card" title={t.familyTitle} blurb={t.familyBlurb} mark={
            <view className="brand-gradient grid size-11 shrink-0 place-items-center rounded-full text-brand-foreground"><Icon name="HeartHandshake" size={22} /></view>} />
          {data.channelLinked ? (
            <NavRow data-testid="sharing-card" title={t.sharingTitle} blurb={t.sharingBlurb} mark={
              <view className="grid size-11 shrink-0 place-items-center rounded-full bg-primary/10 text-primary"><Icon name="Users" size={22} /></view>} />
          ) : null}
        </column>
      </scroll>
    </page>
  );
}
