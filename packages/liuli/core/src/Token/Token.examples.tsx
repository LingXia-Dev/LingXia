import { useLocal } from '@lingxia/view';
import { Held, unused } from '../example';
import { Token, TokenList } from './Token';

/** People a note is shared with: each can be taken away. */
export function Recipients() {
  const people = useLocal<readonly string[]>('people', ['Ana Lima', 'Kai Chen', 'Mira Patel']);
  return (
    <view className="gap-2">
      <TokenList label="Shared with" data-testid="shared">
        {people.value.map(name => (
          <Token key={name} label={name} startIcon="user" onRemove={() => people.set(people.value.filter(each => each !== name))} data-testid={`person-${name.split(' ')[0].toLowerCase()}`} />
        ))}
      </TokenList>
      <text role="status" className="text-footnote text-muted">{`${people.value.length} people`}</text>
    </view>
  );
}

/** Filters that toggle: the ones applied are in the accent's soft step and say they are pressed. */
export function Filters() {
  const on = useLocal<readonly string[]>('on', ['Unread']);
  const toggle = (name: string) => on.set(on.value.includes(name) ? on.value.filter(each => each !== name) : [...on.value, name]);
  return (
    <TokenList label="Filters">
      {['Unread', 'Shared', 'With photos', 'Archived'].map(name => (
        <Token key={name} label={name} isSelected={on.value.includes(name)} onClick={() => toggle(name)} data-testid={`filter-${name.toLowerCase().replace(' ', '-')}`} />
      ))}
    </TokenList>
  );
}

/** Labels keep their hue wherever they appear; removing one takes it off the issue. */
export function Labels() {
  const labels = useLocal('labels', [
    { name: 'bug', hue: 5 as const },
    { name: 'design', hue: 6 as const },
    { name: 'needs triage', hue: 4 as const },
    { name: 'mobile', hue: 2 as const },
  ]);
  return (
    <TokenList label="Labels" data-testid="labels">
      {labels.value.map(each => (
        <Token
          key={each.name}
          label={each.name}
          hue={each.hue}
          size="sm"
          onRemove={() => labels.set(labels.value.filter(other => other.name !== each.name))}
          data-testid={`label-${each.name.replace(/ /g, '-')}`}
        />
      ))}
    </TokenList>
  );
}

export function Sizes() {
  return (
    <Held value={['sm', 'md'] as readonly string[]}>
      {(left, set) => (
        <view className="flex-row flex-wrap items-center gap-2">
          <Token label="Design" size="sm" />
          <Token label="Design" />
          {left.includes('sm') && <Token label="Design" size="sm" removeLabel="Remove Design" onRemove={() => set(left.filter(size => size !== 'sm'))} />}
          {left.includes('md') && <Token label="Design" removeLabel="Remove Design" onRemove={() => set(left.filter(size => size !== 'md'))} />}
        </view>
      )}
    </Held>
  );
}

export function StateDisabled() {
  return <Token label="Locked" removeLabel="Remove Locked" onRemove={unused} isDisabled />;
}

/** The home tile. */
export function Specimen() {
  return (
    <view className="flex-row gap-1.5">
      <Token label="Travel" size="sm" />
      <Token label="2026" size="sm" />
    </view>
  );
}
