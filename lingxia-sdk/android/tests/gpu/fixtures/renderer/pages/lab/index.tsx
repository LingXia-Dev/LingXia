import { useLocal, useView } from '@lingxia/ui';

// The interactive renderer lab: tabs, editors, controls, a keyed grid and a virtual list.
export function render() {
  const { env } = useView();
  const count = useLocal('count', 0);
  const greet = (step: number) => count.set(count.value + step);
  const dark = useLocal('dark', env.theme === 'dark');
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
  const ink = dark.value ? '#F1F5FF' : '#18243A';
  const muted = dark.value ? '#A8B5CF' : '#63718A';
  const surface = dark.value ? '#202D45' : '#FFFFFF';
  const background = dark.value ? '#121C30' : '#F6F7FB';
  const accent = dark.value ? '#ABBBFF' : '#435DD9';
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
    <column testId="showcase-gpu" style={{ backgroundColor: background }}>
      <spacer style={{ height: env.insets.top }} />
      <row style={{ flexGrow: 1, flexBasis: 0, justifyContent:'center'}}>
      <spacer style={{ width: env.insets.left }} />
      <column style={{ flexGrow: 1, flexBasis: 0, maxWidth: 720, padding: pad, gap: compact ? 10 : 20 }}>
      <row style={{ gap: 12, alignItems: 'center' }}>
        <column style={{ flexGrow: 1, flexBasis: 0, gap: 4 }}>
          <text style={{ fontSize: 13, fontFamily:'monospace', lineHeight:'18px', color: accent }}>LINGXIA / SHOWCASE</text>
          <text style={{ fontSize: compact ? 26 : 34, fontWeight: 700, lineHeight: compact ? 34 : 42, color: ink }}>Small moments.</text>
        </column>
        <button testId="theme" onClick={() => dark.set(!dark.value)} style={{ backgroundColor: surface, color: ink, fontSize: 14, padding: 12, minHeight: 44, flexShrink: 0 }}>
          {dark.value ? 'Light' : 'Dark'}
        </button>
      </row>
      <row style={{ gap: 8 }}>
        <button testId="open-todo" onClick={() => greet(1)} style={{ flexGrow: 1, flexBasis: 0, backgroundColor: surface, color: accent, minHeight: 44 }}>ToDo</button>
        <button testId="open-clipboard" onClick={() => greet(1)} style={{ flexGrow: 1, flexBasis: 0, backgroundColor: surface, color: accent, minHeight: 44 }}>Clipboard</button>
        <button testId="open-share" onClick={() => greet(1)} style={{ flexGrow: 1, flexBasis: 0, backgroundColor: surface, color: accent, minHeight: 44 }}>Share</button>
      </row>
      <row style={{ gap: 10 }}>
        <button key="hello" testId="tab-hello" onClick={() => tab.set('hello')} style={{ flexGrow: 1, flexBasis: 0, backgroundColor: tab.value === 'hello' ? '#435DD9' : surface, color: tab.value === 'hello' ? '#FFFFFF' : ink }}>Hello world</button>
        <button key="explore" testId="tab-explore" onClick={() => tab.set('explore')} style={{ flexGrow: 1, flexBasis: 0, backgroundColor: tab.value === 'explore' ? '#435DD9' : surface, color: tab.value === 'explore' ? '#FFFFFF' : ink }}>Explore</button>
        <button key="discover" testId="tab-discover" onClick={() => tab.set('discover')} style={{ flexGrow: 1, flexBasis: 0, backgroundColor: tab.value === 'discover' ? '#435DD9' : surface, color: tab.value === 'discover' ? '#FFFFFF' : ink }}>Discover</button>
      </row>
      {tab.value === 'hello' ? (
        <scroll key="hello-card" style={{ flexGrow: 1, flexBasis: 0, padding: compact ? 18 : 24, gap: compact ? 10 : 18, backgroundColor: surface, borderRadius: 22 }}>
          {!compact && <text style={{ fontSize: 13, color: accent }}>01 / CONNECTION</text>}
          <row style={{ gap: 8, alignItems: 'center' }}>
            <text style={{ flexGrow: 1, flexBasis: 0, fontSize: compact ? 24 : 30, fontWeight: 600, color: ink }}>Hello, world!</text>
            <button testId="note-mode" onClick={() => longNote.set(!longNote.value)} style={{ backgroundColor: surface, color: accent, padding: 8, fontSize: 12 }}>{longNote.value ? 'Short hello' : 'Long note'}</button>
          </row>
          {longNote.value
            ? <textarea key="greeting-note" testId="greeting-input" bind={greeting} placeholder="A little more to say…" style={{ backgroundColor: background, color: ink, fontSize: 18, fontFamily:'monospace', lineHeight:'28px', height: 112 }} />
            : <input key="greeting" testId="greeting-input" bind={greeting} placeholder="Write a little hello" onSubmit={value => sentGreeting.set(value)} style={{ backgroundColor: background, color: ink, fontSize: 18 }} />}
          {!compact && <text style={{ fontSize: 17, lineHeight:'25px', color: muted }}>One tap. A little more warmth.</text>}
          <button testId="say-hello" onClick={() => greet(1)} style={{ backgroundColor: '#435DD9', color: '#FFFFFF', padding: compact ? 12 : 18, fontSize: 18, borderRadius: 14 }}>Say hello</button>
          <text testId="hello-count" style={{ fontSize: 15, color: muted }}>{count.value === 0 ? 'Your first hello starts here.' : `Hello received ${count.value} ${count.value === 1 ? 'time' : 'times'}.`}</text>
          <text testId="greeting-preview" style={{ fontSize: 14, color: muted }}>{greeting.value ? `Your note: ${greeting.value}` : 'Make it personal.'}</text>
          <button testId="opacity-class-toggle" onClick={() => faded.set(!faded.value)} style={{ minHeight: 44, backgroundColor: background, color: ink }}>{faded.value ? 'Show full color' : 'Fade together'}</button>
          <stack testId="opacity-sample" className="w-40 h-30 bg-white">
            <stack className={faded.value ? 'w-30 h-25 bg-[#ff0000] opacity-50' : 'w-30 h-25 bg-[#ff0000] opacity-100'}>
              <spacer className="w-20 h-20 bg-[#0000ff]" />
            </stack>
            <spacer className="w-15 h-10 bg-[#00ff00] opacity-50" style={{ left:60,top:60}} />
          </stack>
          {sentGreeting.value && <text testId="greeting-sent" style={{ fontSize: 14, color: accent }}>{`Saved: ${sentGreeting.value}`}</text>}
        </scroll>
      ) : tab.value === 'explore' ? (
        <scroll key="explore-card" testId="explore-scroll" style={{ flexGrow: 1, flexBasis: 0, padding: compact ? 16 : 24, gap: compact ? 10 : 18, backgroundColor: surface, borderRadius: 22 }}>
          <text style={{ fontSize: 24, fontWeight: 600, color: ink }}>Made to respond.</text>
          {!compact && <text style={{ fontSize: 16, color: muted }}>Try a new look, switch tabs, or start fresh. Your hello stays with you.</text>}
          <text key="moment-story" testId="moment-story" style={{ lineClamp: storyExpanded.value ? undefined : 2, fontSize: 16, lineHeight:'24px', color: muted }}>A small hello can change the shape of a day. Leave a thoughtful note, take a quiet moment, or share something that made you smile. Little gestures travel further than we think. Keep a little room for curiosity, and come back whenever you need a fresh start.</text>
          <button testId="story-toggle" onClick={() => storyExpanded.set(!storyExpanded.value)} style={{ backgroundColor: background, color: ink, minHeight: 44 }}>{storyExpanded.value ? 'Show less' : 'Read the whole story'}</button>
          <stack key="greeting-cards" testId="greeting-cards" style={{ height: 128 }}>
            <button testId="stack-back" onClick={() => greet(1)} style={{ width: 240, height: 64, backgroundColor: dark.value ? '#33496A' : '#E1E8FA', color: ink, borderRadius: 16 }}>A little hello</button>
            <button testId="stack-front" onClick={() => greet(2)} style={{ width: 200, height: 64, left:24,top:48, backgroundColor: '#435DD9', color: '#FFFFFF', borderRadius: 16 }}>Pass it on</button>
          </stack>
          <button testId="reset-count" disabled={!(count.value > 0)} onClick={() => count.set(0)} style={{ backgroundColor: count.value > 0 ? '#435DD9' : background, color: count.value > 0 ? '#FFFFFF' : muted, padding: 16 }}>Reset greetings</button>
          {!compact && <button testId="appearance-auto" onClick={() => dark.set(env.theme === 'dark')} style={{ backgroundColor: background, color: ink, padding: 12 }}>Use device theme</button>}
          <checkbox testId="keep-notes" bind={keepNotes} style={{ color: ink, backgroundColor: background, borderRadius: 12, fontSize: 16 }}>Keep my little notes</checkbox>
          <switch testId="reminders" bind={reminders} style={{ color: ink, backgroundColor: background, borderRadius: 12, fontSize: 16 }}>A daily moment</switch>
          <switch testId="reminders-disabled" value={true} disabled style={{ color: muted, backgroundColor: background, borderRadius: 12, fontSize: 16 }}>Coming soon</switch>
          <text testId="preferences-status" style={{ fontSize: 14, color: muted }}>{`${keepNotes.value ? 'Notes kept' : 'Notes off'} / ${reminders.value ? 'Daily moment on' : 'Daily moment off'}`}</text>
          <row style={{ gap: 12, alignItems: 'center' }}>
            <text style={{ flexGrow: 1, flexBasis: 0, fontSize: 20, fontWeight: 600, color: ink }}>A little time for you</text>
            <text testId="pause-status" style={{ color: accent, fontSize: 16 }}>{`${pauseMinutes.value} min`}</text>
          </row>
          <slider key="pause-length" testId="pause-length" accessibilityLabel="Minutes for a pause" bind={pauseMinutes} min={0} max={10} step={3} disabled={!(pauseEditable.value)} style={{ color: accent }} />
          <button testId="pause-toggle" onClick={() => pauseEditable.set(!pauseEditable.value)} style={{ color: ink, backgroundColor: background }}>{pauseEditable.value ? 'Keep this duration' : 'Adjust duration'}</button>
          <text style={{ fontSize: 20, fontWeight: 600, color: ink }}>Your rhythm</text>
          <radio key="pace" testId="pace-group" accessibilityLabel="Your rhythm" bind={pace} disabled={!(choicesEnabled.value)}
            options={[
              { value: 'daily', label: 'A little inspiration every day', testId: 'pace-daily' },
              { value: 'weekly', label: 'A thoughtful weekly collection', testId: 'pace-weekly' },
              { value: 'monthly', label: 'Monthly editions - coming soon', disabled: true, testId: 'pace-monthly' },
              { value: 'quiet', label: 'Only when I choose to explore', testId: 'pace-quiet' },
            ]} style={{ fontSize: 16, color: ink, backgroundColor: background, gap: 4, padding: 8, borderRadius: 14 }} />
          <text testId="pace-status" style={{ fontSize: 14, color: muted }}>{`Your rhythm: ${pace.value}`}</text>
          <button testId="pace-toggle" onClick={() => choicesEnabled.set(!choicesEnabled.value)} style={{ color: ink, backgroundColor: background }}>{choicesEnabled.value ? 'Pause choices' : 'Edit choices'}</button>
          <text testId="explore-count" style={{ fontSize: 15, color: muted }}>{`${count.value} greetings shared`}</text>
          <grid key="moments-grid" testId="moments-grid" style={{ gap: 14, gridTemplateColumns: env.viewport.width >= 600 ? 2 : 1 }}>
          {Array.from({ length: 20 }, (_, index) => (
            <column key={`moment-${index}`} style={{ backgroundColor: background, padding: 18, gap: 10, borderRadius: 14 }}>
              <text style={{ fontSize: 20, color: ink }}>{`Moment ${String(index + 1).padStart(2, '0')}`}</text>
              <text style={{ fontSize: 14, color: muted }}>A small hello can make someone's day.</text>
              <button testId={`moment-${index + 1}`} onClick={() => greet(1)} style={{ backgroundColor: '#435DD9', color: '#FFFFFF', padding: 12, minHeight: 44 }}>Send a hello</button>
            </column>
          ))}
          </grid>
        </scroll>
      ) : (
        <column key="discover-card" style={{ flexGrow: 1, flexBasis: 0, gap: 10 }}>
          <row style={{ gap: 8, alignItems: 'center' }}>
            <search key="moment-search" testId="moment-search" bind={query} onSubmit={applySearch} placeholder="Find a moment by number" style={{ flexGrow: 1, flexBasis: 0, backgroundColor: surface, color: ink, fontSize: 16 }} />
            {query.value && <button testId="clear-search" onClick={() => { query.set(''); applySearch(''); }} style={{ backgroundColor: surface, color: accent, padding: 12 }}>Clear</button>}
          </row>
          {!compact && <row style={{ gap: 10 }}>
            <button testId="discover-first" disabled={!(resultCount > 0)} onClick={() => anchor.set({ index: 0, align: 'start', seq: (anchor.value?.seq ?? 0) + 1 })} style={{ flexGrow: 1, flexBasis: 0, backgroundColor: surface, color: ink }}>First moment</button>
            <button testId="discover-last" disabled={!(resultCount > 0)} onClick={() => anchor.set({ index: resultCount - 1, align: 'end', seq: (anchor.value?.seq ?? 0) + 1 })} style={{ flexGrow: 1, flexBasis: 0, backgroundColor: surface, color: ink }}>Latest moment</button>
          </row>}
          <text testId="discover-status" style={{ fontSize: 13, color: muted }}>{appliedQuery.value ? `${resultCount} ${resultCount === 1 ? 'moment' : 'moments'} found for "${appliedQuery.value}"` : latest.value ? 'You are up to date.' : 'More small moments below.'}</text>
          {resultCount === 0 && <text testId="search-empty" style={{ padding: 20, color: muted, fontSize: 16 }}>No moments found. Try another number.</text>}
          <list key="discover" testId="discover-list" count={resultCount} anchor={anchor.value ?? undefined} onReachStart={() => latest.set(false)} onReachEnd={() => latest.set(true)}
            style={{ flexGrow: 1, flexBasis: 0, gap: 14, padding: 12, backgroundColor: surface, borderRadius: 22 }}
            row={index => <MomentCard key={`discovery-${results.value?.[index] ?? index}`} index={results.value?.[index] ?? index} ink={ink} muted={muted} background={background} onHello={() => greet(1)} />} />
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
    <column style={{ padding: 18, gap: 10, backgroundColor: background, borderRadius: 14 }}>
      <row style={{ gap: 12, alignItems: 'center' }}>
        <image testId={`discovery-image-${index + 1}`} src="public/mark.png" style={{ objectFit: 'cover', width: 48, height: 48, borderRadius: 16, backgroundColor: '#E3E9FA' }} />
        <column style={{ flexGrow: 1, flexBasis: 0, gap: 4 }}>
          <text testId={`discovery-title-${index + 1}`} style={{ fontSize: 20, color: ink }}>{`Moment ${String(index + 1).padStart(3, '0')}`}</text>
          <text style={{ fontSize: 12, color: muted }}>THE SMALL MOMENTS CLUB</text>
        </column>
      </row>
      <text style={{ fontSize: 14, color: muted }}>{index % 3 === 0 ? 'Take a little time to notice something beautiful today.' : 'A small hello can make someone\'s day.'}</text>
      {saved.value && <text style={{ fontSize: 13, color: muted }}>A little reminder, kept for you.</text>}
      <button testId={`discovery-save-${index + 1}`} onClick={() => saved.set(!saved.value)} style={{ backgroundColor: saved.value ? '#435DD9' : '#E3E9FA', color: saved.value ? '#FFFFFF' : '#243A73', padding: 12 }}>{saved.value ? 'Saved to your day' : 'Keep this moment'}</button>
      <button testId={`discovery-hello-${index + 1}`} onClick={onHello} style={{ backgroundColor: background, color: ink, padding: 8 }}>Send a hello</button>
    </column>
  );
}
