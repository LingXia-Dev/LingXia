import { Said } from '../example';
import { Button } from '../Button/Button';
import { Item, ItemList } from '../Item/Item';
import { Section } from './Section';

/** A part of a page with its heading, a line about it and a way to see more. */
export function Recent() {
  return (
    <Said>
      {say => (
        <Section title="Recent notes" description="Edited in the last week" action={<Button label="See all" variant="link" size="sm" onClick={() => say('Opened all recent notes')} />} data-testid="recent">
          <ItemList label="Recent notes">
            <Item label="Trip to the coast" description="Edited yesterday" hasChevron onClick={() => say('Opened Trip to the coast')} />
            <Item label="Reading list" description="Edited on Monday" hasChevron onClick={() => say('Opened Reading list')} />
          </ItemList>
        </Section>
      )}
    </Said>
  );
}

/** Sections inside a section step their headings down. */
export function Nested() {
  return (
    <Section title="Notifications" data-testid="outer">
      <Section title="Email" headingLevel={3} description="Sent to ana@example.com" data-testid="inner">
        <text className="text-body text-muted">A digest every Monday.</text>
      </Section>
      <Section title="On this device" headingLevel={3}>
        <text className="text-body text-muted">Mentions, at once.</text>
      </Section>
    </Section>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <Section title="Recent" className="w-28">
      <view className="h-6 rounded-control bg-text/8" />
    </Section>
  );
}
