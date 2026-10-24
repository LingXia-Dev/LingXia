import { trackingFixture } from './tracking-fixture';
import { flexFixture } from './flex-fixture';
import { italicFixture } from './italic-fixture';
import { paddedTextFixture } from './padded-text-fixture';
import { strikeFixture } from './strike-fixture';
import { percentFixture } from './percent-fixture';
import { vectorFixture } from './vector-fixture';
import { shadowFixture } from './shadow-fixture';
import { gradientFixture } from './gradient-fixture';
import { useLocal, type View } from '@lingxia/ui';
import { borderFixture } from './border-fixture';
import { alignmentFixture } from './alignment-fixture';

export function render({ data, actions, env }: View) {
  if (data.rendererFixture === 'flex') return flexFixture(env);
  if (data.rendererFixture === 'tracking') return trackingFixture(env);
  if (data.rendererFixture === 'italic') return italicFixture(env);
  if (data.rendererFixture === 'padded-text') return paddedTextFixture(env);
  if (data.rendererFixture === 'strike') return strikeFixture(env);
  if (data.rendererFixture === 'vector') return vectorFixture(env);
  if (data.rendererFixture === 'percent') return percentFixture(env);
  if (data.rendererFixture === 'shadow') return shadowFixture(env);
  if (data.rendererFixture === 'gradient') return gradientFixture(env);
  if (data.rendererFixture === 'alignment') return alignmentFixture(env);
  if (data.rendererFixture === 'border') return borderFixture(env);
  const count = { value: Number(data.count ?? 0) };
  const dark = env.theme === 'dark';
  const tab = useLocal('tab', 'hello');
  const latest = useLocal('latest', false);
  const query = useLocal('query', '');
  const appliedQuery = useLocal('applied-query', '');
  const results = useLocal<number[] | null>('search-results', null);
  const reminders = useLocal('reminders', false);
  const storyExpanded = useLocal('story-expanded', false);
  const pauseMinutes = useLocal('pause-minutes', 3);
  const pauseEditable = useLocal('pause-editable', true);
  const pace = useLocal('pace', 'weekly');
  const choicesEnabled = useLocal('choices-enabled', true);
  const keepNotes = useLocal('keep-notes', true);
  const greeting = useLocal('greeting', '');
  const sentGreeting = useLocal('sent-greeting', '');
  const longNote = useLocal('long-note', false);
  const faded = useLocal('faded', true);
  const compact = env.viewport.height < 480;
  const anchor = useLocal<{ index: number; align: 'start' | 'end'; seq: number } | null>('discover-anchor', null);
  const ink = dark ? '#F1F5FF' : '#18243A';
  const muted = dark ? '#A8B5CF' : '#63718A';
  const surface = dark ? '#202D45' : '#FFFFFF';
  const background = dark ? '#121C30' : '#F6F7FB';
  const accent = dark ? '#ABBBFF' : '#435DD9';
  const pad = compact ? 18 : 26;
  const resultCount = results.value?.length ?? 1000;
  const applySearch = (value: string) => {
    const term = value.trim().toLowerCase();
    appliedQuery.set(term);
    results.set(term ? Array.from({ length: 1000 }, (_, index) => index).filter(index => `moment ${String(index + 1).padStart(3, '0')}`.includes(term)) : null);
    anchor.set(null);
    latest.set(false);
  };

  return (
    <column testId="showcase-gpu" style={{ background }}>
      <spacer style={{ height: env.insets.top }} />
      <row style={{ grow: 1, flexBasis: 0, justify: 'center' }}>
      <spacer style={{ width: env.insets.left }} />
      <column style={{ grow: 1, flexBasis: 0, maxWidth: 720, padding: pad, gap: compact ? 10 : 20 }}>
      <row style={{ gap: 12, align: 'center' }}>
        <column style={{ grow: 1, flexBasis: 0, gap: 4 }}>
          <text style={{ fontSize: 13, fontFamily: 'mono', lineHeight: 18, color: accent }}>LINGXIA / SHOWCASE</text>
          <text style={{ fontSize: compact ? 26 : 34, fontWeight: 700, lineHeight: compact ? 34 : 42, color: ink }}>Small moments.</text>
        </column>
        <button testId="theme" onClick={() => actions.setAppearance({ preference: dark ? 'light' : 'dark' })} style={{ background: surface, color: ink, fontSize: 14, padding: 12, minHeight: 44, shrink: 0 }}>
          {dark ? 'Light' : 'Dark'}
        </button>
      </row>
      <row style={{ gap: 8 }}>
        <button testId="open-todo" onClick={() => actions.openTodo()} style={{ grow: 1, flexBasis: 0, background: surface, color: accent, minHeight: 44 }}>ToDo</button>
        <button testId="open-clipboard" onClick={() => actions.openClipboard()} style={{ grow: 1, flexBasis: 0, background: surface, color: accent, minHeight: 44 }}>Clipboard</button>
        <button testId="open-share" onClick={() => actions.openShare()} style={{ grow: 1, flexBasis: 0, background: surface, color: accent, minHeight: 44 }}>Share</button>
      </row>
      <row style={{ gap: 10 }}>
        <button key="hello" testId="tab-hello" onClick={() => tab.set('hello')} style={{ grow: 1, flexBasis: 0, background: tab.value === 'hello' ? '#435DD9' : surface, color: tab.value === 'hello' ? '#FFFFFF' : ink }}>Hello world</button>
        <button key="explore" testId="tab-explore" onClick={() => tab.set('explore')} style={{ grow: 1, flexBasis: 0, background: tab.value === 'explore' ? '#435DD9' : surface, color: tab.value === 'explore' ? '#FFFFFF' : ink }}>Explore</button>
        <button key="discover" testId="tab-discover" onClick={() => tab.set('discover')} style={{ grow: 1, flexBasis: 0, background: tab.value === 'discover' ? '#435DD9' : surface, color: tab.value === 'discover' ? '#FFFFFF' : ink }}>Discover</button>
      </row>
      {tab.value === 'hello' ? (
        <scroll key="hello-card" style={{ grow: 1, flexBasis: 0, padding: compact ? 18 : 24, gap: compact ? 10 : 18, background: surface, radius: 22 }}>
          {!compact && <text style={{ fontSize: 13, color: accent }}>01 / CONNECTION</text>}
          <row style={{ gap: 8, align: 'center' }}>
            <text style={{ grow: 1, flexBasis: 0, fontSize: compact ? 24 : 30, fontWeight: 600, color: ink }}>Hello, world!</text>
            <button testId="note-mode" onClick={() => longNote.set(!longNote.value)} style={{ background: surface, color: accent, padding: 8, fontSize: 12 }}>{longNote.value ? 'Short hello' : 'Long note'}</button>
          </row>
          {longNote.value
            ? <textarea key="greeting-note" testId="greeting-input" bind={greeting} placeholder="A little more to say…" style={{ background, color: ink, fontSize: 18, fontFamily: 'mono', lineHeight: 28, height: 112 }} />
            : <input key="greeting" testId="greeting-input" bind={greeting} placeholder="Write a little hello" onSubmit={value => sentGreeting.set(value)} style={{ background, color: ink, fontSize: 18 }} />}
          {!compact && <text style={{ fontSize: 17, lineHeight: 25, color: muted }}>One tap. A little more warmth.</text>}
          <button testId="say-hello" onClick={() => actions.increment({ step: 1 })} style={{ background: '#435DD9', color: '#FFFFFF', padding: compact ? 12 : 18, fontSize: 18, radius: 14 }}>Say hello</button>
          <text testId="hello-count" style={{ fontSize: 15, color: muted }}>{count.value === 0 ? 'Your first hello starts here.' : `Hello received ${count.value} ${count.value === 1 ? 'time' : 'times'}.`}</text>
          <text testId="greeting-preview" style={{ fontSize: 14, color: muted }}>{greeting.value ? `Your note: ${greeting.value}` : 'Make it personal.'}</text>
          <button testId="opacity-class-toggle" onClick={() => faded.set(!faded.value)} style={{ minHeight: 44, background, color: ink }}>{faded.value ? 'Show full color' : 'Fade together'}</button>
          <stack testId="opacity-sample" className="w-40 h-30 bg-white">
            <stack className={faded.value ? 'w-30 h-25 bg-[#ff0000] opacity-50' : 'w-30 h-25 bg-[#ff0000] opacity-100'}>
              <spacer className="w-20 h-20 bg-[#0000ff]" />
            </stack>
            <spacer className="w-15 h-10 bg-[#00ff00] opacity-50" style={{ offset: { x: 60, y: 60 } }} />
          </stack>
          {sentGreeting.value && <text testId="greeting-sent" style={{ fontSize: 14, color: accent }}>{`Saved: ${sentGreeting.value}`}</text>}
        </scroll>
      ) : tab.value === 'explore' ? (
        <scroll key="explore-card" testId="explore-scroll" style={{ grow: 1, flexBasis: 0, padding: compact ? 16 : 24, gap: compact ? 10 : 18, background: surface, radius: 22 }}>
          <text style={{ fontSize: 24, fontWeight: 600, color: ink }}>Made to respond.</text>
          {!compact && <text style={{ fontSize: 16, color: muted }}>Try a new look, switch tabs, or start fresh. Your hello stays with you.</text>}
          <text key="moment-story" testId="moment-story" lines={storyExpanded.value ? undefined : 2} style={{ fontSize: 16, lineHeight: 24, color: muted }}>A small hello can change the shape of a day. Leave a thoughtful note, take a quiet moment, or share something that made you smile. Little gestures travel further than we think. Keep a little room for curiosity, and come back whenever you need a fresh start.</text>
          <button testId="story-toggle" onClick={() => storyExpanded.set(!storyExpanded.value)} style={{ background, color: ink, minHeight: 44 }}>{storyExpanded.value ? 'Show less' : 'Read the whole story'}</button>
          <stack key="greeting-cards" testId="greeting-cards" style={{ height: 128 }}>
            <button testId="stack-back" onClick={() => actions.increment({ step: 1 })} style={{ width: 240, height: 64, background: dark ? '#33496A' : '#E1E8FA', color: ink, radius: 16 }}>A little hello</button>
            <button testId="stack-front" onClick={() => actions.increment({ step: 2 })} style={{ width: 200, height: 64, offset: { x: 24, y: 48 }, background: '#435DD9', color: '#FFFFFF', radius: 16 }}>Pass it on</button>
          </stack>
          <button testId="reset-count" enabled={count.value > 0} onClick={() => actions.resetGreetings()} style={{ background: count.value > 0 ? '#435DD9' : background, color: count.value > 0 ? '#FFFFFF' : muted, padding: 16 }}>Reset greetings</button>
          {!compact && <button testId="appearance-auto" onClick={() => actions.setAppearance({ preference: 'auto' })} style={{ background, color: ink, padding: 12 }}>Use device theme</button>}
          <checkbox testId="keep-notes" bind={keepNotes} style={{ color: ink, background, radius: 12, fontSize: 16 }}>Keep my little notes</checkbox>
          <switch testId="reminders" bind={reminders} style={{ color: ink, background, radius: 12, fontSize: 16 }}>A daily moment</switch>
          <switch testId="reminders-disabled" value={true} enabled={false} style={{ color: muted, background, radius: 12, fontSize: 16 }}>Coming soon</switch>
          <text testId="preferences-status" style={{ fontSize: 14, color: muted }}>{`${keepNotes.value ? 'Notes kept' : 'Notes off'} / ${reminders.value ? 'Daily moment on' : 'Daily moment off'}`}</text>
          <row style={{ gap: 12, align: 'center' }}>
            <text style={{ grow: 1, flexBasis: 0, fontSize: 20, fontWeight: 600, color: ink }}>A little time for you</text>
            <text testId="pause-status" style={{ color: accent, fontSize: 16 }}>{`${pauseMinutes.value} min`}</text>
          </row>
          <slider key="pause-length" testId="pause-length" accessibilityLabel="Minutes for a pause" bind={pauseMinutes} min={0} max={10} step={3} enabled={pauseEditable.value} style={{ color: accent }} />
          <button testId="pause-toggle" onClick={() => pauseEditable.set(!pauseEditable.value)} style={{ color: ink, background }}>{pauseEditable.value ? 'Keep this duration' : 'Adjust duration'}</button>
          <text style={{ fontSize: 20, fontWeight: 600, color: ink }}>Your rhythm</text>
          <radio key="pace" testId="pace-group" accessibilityLabel="Your rhythm" bind={pace} enabled={choicesEnabled.value}
            options={[
              { value: 'daily', label: 'A little inspiration every day', testId: 'pace-daily' },
              { value: 'weekly', label: 'A thoughtful weekly collection', testId: 'pace-weekly' },
              { value: 'monthly', label: 'Monthly editions - coming soon', enabled: false, testId: 'pace-monthly' },
              { value: 'quiet', label: 'Only when I choose to explore', testId: 'pace-quiet' },
            ]} style={{ fontSize: 16, color: ink, background, gap: 4, padding: 8, radius: 14 }} />
          <text testId="pace-status" style={{ fontSize: 14, color: muted }}>{`Your rhythm: ${pace.value}`}</text>
          <button testId="pace-toggle" onClick={() => choicesEnabled.set(!choicesEnabled.value)} style={{ color: ink, background }}>{choicesEnabled.value ? 'Pause choices' : 'Edit choices'}</button>
          <text testId="explore-count" style={{ fontSize: 15, color: muted }}>{`${count.value} greetings shared`}</text>
          <grid key="moments-grid" testId="moments-grid" columns={env.viewport.width >= 600 ? 2 : 1} style={{ gap: 14 }}>
          {Array.from({ length: 20 }, (_, index) => (
            <column key={`moment-${index}`} style={{ background, padding: 18, gap: 10, radius: 14 }}>
              <text style={{ fontSize: 20, color: ink }}>{`Moment ${String(index + 1).padStart(2, '0')}`}</text>
              <text style={{ fontSize: 14, color: muted }}>A small hello can make someone's day.</text>
              <button testId={`moment-${index + 1}`} onClick={() => actions.increment({ step: 1 })} style={{ background: '#435DD9', color: '#FFFFFF', padding: 12, minHeight: 44 }}>Send a hello</button>
            </column>
          ))}
          </grid>
        </scroll>
      ) : (
        <column key="discover-card" style={{ grow: 1, flexBasis: 0, gap: 10 }}>
          <row style={{ gap: 8, align: 'center' }}>
            <search key="moment-search" testId="moment-search" bind={query} onSubmit={applySearch} placeholder="Find a moment by number" style={{ grow: 1, flexBasis: 0, background: surface, color: ink, fontSize: 16 }} />
            {query.value && <button testId="clear-search" onClick={() => { query.set(''); applySearch(''); }} style={{ background: surface, color: accent, padding: 12 }}>Clear</button>}
          </row>
          {!compact && <row style={{ gap: 10 }}>
            <button testId="discover-first" enabled={resultCount > 0} onClick={() => anchor.set({ index: 0, align: 'start', seq: (anchor.value?.seq ?? 0) + 1 })} style={{ grow: 1, flexBasis: 0, background: surface, color: ink }}>First moment</button>
            <button testId="discover-last" enabled={resultCount > 0} onClick={() => anchor.set({ index: resultCount - 1, align: 'end', seq: (anchor.value?.seq ?? 0) + 1 })} style={{ grow: 1, flexBasis: 0, background: surface, color: ink }}>Latest moment</button>
          </row>}
          <text testId="discover-status" style={{ fontSize: 13, color: muted }}>{appliedQuery.value ? `${resultCount} ${resultCount === 1 ? 'moment' : 'moments'} found for "${appliedQuery.value}"` : latest.value ? 'You are up to date.' : 'More small moments below.'}</text>
          {resultCount === 0 && <text testId="search-empty" style={{ padding: 20, color: muted, fontSize: 16 }}>No moments found. Try another number.</text>}
          <list key="discover" testId="discover-list" count={resultCount} anchor={anchor.value ?? undefined} onReachStart={() => latest.set(false)} onReachEnd={() => latest.set(true)}
            style={{ grow: 1, flexBasis: 0, gap: 14, padding: 12, background: surface, radius: 22 }}
            row={index => <MomentCard key={`discovery-${results.value?.[index] ?? index}`} index={results.value?.[index] ?? index} ink={ink} muted={muted} background={background} onHello={() => actions.increment({ step: 1 })} />} />
        </column>
      )}
      {!compact && <column style={{ gap: 8, padding: 4 }}>
        <text style={{ fontSize: 15, color: ink }}>Built for the way you move.</text>
        <text style={{ fontSize: 14, color: muted }}>Turn your phone. Pick up where you left off.</text>
      </column>}
      </column>
      <spacer style={{ width: env.insets.right }} />
      </row>
      <spacer style={{ height: env.insets.bottom }} />
    </column>
  );
}

function MomentCard({ index, ink, muted, background, onHello }: { index: number; ink: string; muted: string; background: string; onHello: () => void }) {
  const saved = useLocal('saved', false);
  return (
    <column style={{ padding: 18, gap: 10, background, radius: 14 }}>
      <row style={{ gap: 12, align: 'center' }}>
        <image testId={`discovery-image-${index + 1}`} src="public/mark.png" fit="cover" style={{ width: 48, height: 48, radius: 16, background: '#E3E9FA' }} />
        <column style={{ grow: 1, flexBasis: 0, gap: 4 }}>
          <text testId={`discovery-title-${index + 1}`} style={{ fontSize: 20, color: ink }}>{`Moment ${String(index + 1).padStart(3, '0')}`}</text>
          <text style={{ fontSize: 12, color: muted }}>THE SMALL MOMENTS CLUB</text>
        </column>
      </row>
      <text style={{ fontSize: 14, color: muted }}>{index % 3 === 0 ? 'Take a little time to notice something beautiful today.' : 'A small hello can make someone\'s day.'}</text>
      {saved.value && <text style={{ fontSize: 13, color: muted }}>A little reminder, kept for you.</text>}
      <button testId={`discovery-save-${index + 1}`} onClick={() => saved.set(!saved.value)} style={{ background: saved.value ? '#435DD9' : '#E3E9FA', color: saved.value ? '#FFFFFF' : '#243A73', padding: 12 }}>{saved.value ? 'Saved to your day' : 'Keep this moment'}</button>
      <button testId={`discovery-hello-${index + 1}`} onClick={onHello} style={{ background, color: ink, padding: 8 }}>Send a hello</button>
    </column>
  );
}
