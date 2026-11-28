import { createContext, cx, useContext, useId, type Child } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { LayerContext } from '../internal/layer';
import { citationRecipe as r } from './Citation.recipe';
import { dataOf } from '../internal/data';

export interface CitationSource {
  /** What a Citation names it by. */
  id: string;
  /** The page or document's title: "Ferry timetable, summer". */
  title: string;
  /** Where it is from, short: "example.com", "Trip notes". The pill shows it, else the title. */
  site?: string;
  /** The words it was cited for, shown on its card. */
  excerpt?: string;
}

interface Cited {
  number: number;
  source: CitationSource;
  cardId: string;
}

const Sources = createContext<{ byId: Readonly<Record<string, Cited>>; testId?: string } | null>(null);

export interface CitationGroupProps {
  /** The sources, in the order they are numbered. */
  sources: readonly CitationSource[];
  /** The cited content: `<text>` with a Citation after each claim, a Markdown, a MessageBubble's text. */
  children: Child;
  /** Runs when a source's pill is pressed: the app opens the page or the document. */
  onOpen: (source: CitationSource) => void;
  /** Names the row of sources for assistive tech. Defaults to "Sources" in the page's language. */
  label?: string;
  className?: string;
  /** On the group; a pill gets `<testid>-source-<id>`, a mark `<testid>-mark-<id>`, a card `<testid>-card-<id>`. */
  'data-testid'?: string;
}

/** An answer that cites its sources: numbered marks among the words, and the sources under them. */
export function CitationGroup({ sources, children, onOpen, label = liuliText('@liuli.citation.sources'), className, 'data-testid': testId, ...attributes }: CitationGroupProps) {
  const base = useId();
  const cited = sources.map((source, index): Cited => ({ number: index + 1, source, cardId: `${base}-${index}` }));
  const byId = Object.fromEntries(cited.map(entry => [entry.source.id, entry]));
  return (
    <view data-testid={testId} {...dataOf(attributes)} className={cx(r.group.base, className)}>
      <Sources.Provider value={{ byId, testId }}><view className={r.content.base}>{children}</view></Sources.Provider>
      {cited.length > 0 && (
        <view role="group" aria-label={label} className={r.sources.base}>
          {cited.map(({ number, source }) => (
            <button
              key={source.id}
              aria-label={liuliText('@liuli.citation.source', { number, title: source.title, hasSite: source.site === undefined ? 'no' : 'yes', site: source.site ?? '' })}
              onClick={() => onOpen(source)}
              data-testid={testId && `${testId}-source-${source.id}`}
              className={r.source.base}
            >
              <text className={r.sourceNumber.base}>{number}</text>
              <text className={r.sourceName.base}>{source.site ?? source.title}</text>
            </button>
          ))}
        </view>
      )}
      {cited.map(({ source, cardId }) => (
        <LayerContext.Provider key={source.id} value="overlay">
          <view id={cardId} popover="hint" role="dialog" aria-label={source.title} data-testid={testId && `${testId}-card-${source.id}`} style={{ positionArea: 'bottom span-inline-end' }} className={r.card.base}>
            <text className={r.cardTitle.base}>{source.title}</text>
            {source.site !== undefined && <text className={r.cardSite.base}>{source.site}</text>}
            {source.excerpt !== undefined && <text className={r.cardExcerpt.base}>{source.excerpt}</text>}
          </view>
        </LayerContext.Provider>
      ))}
    </view>
  );
}

export interface CitationProps {
  /** The `id` of one of the group's sources. */
  source: string;
  /** On the mark; defaults to the group's `<testid>-mark-<source>`. */
  'data-testid'?: string;
}

/** A source's number after the claim it backs, among the words of a `<text>` inside a CitationGroup. */
export function Citation({ source, 'data-testid': testId }: CitationProps) {
  const group = useContext(Sources);
  if (group === null) throw Error(`UI_PROP: Citation "${source}" goes inside a CitationGroup`);
  const entry = Object.prototype.hasOwnProperty.call(group.byId, source) ? group.byId[source] : undefined;
  if (entry === undefined) throw Error(`UI_PROP: Citation "${source}" names no source of its CitationGroup`);
  // A run takes no name of its own: it reads its number, then the card it describes.
  return (
    <link
      interestFor={entry.cardId}
      popoverTarget={entry.cardId}
      aria-describedby={entry.cardId}
      data-testid={testId ?? (group.testId && `${group.testId}-mark-${source}`)}
      className={r.mark.base}
    >
      {/* Held to the word it follows: a mark never starts a line. */}
      {` ${entry.number}`}
    </link>
  );
}
