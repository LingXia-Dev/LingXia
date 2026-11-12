import { useLocal, type VectorPath } from '@lingxia/view';
import { TextInput } from './TextInput';

/** Bound to a View-local cell: no Logic round trip per keystroke. */
export function Bound() {
  const email = useLocal('email', '');
  const invalid = email.value !== '' && !email.value.includes('@');
  return (
    <TextInput
      bind={email}
      label="Email"
      placeholder="you@example.com"
      type="email"
      autoComplete="email"
      autoCapitalize="none"
      enterKeyHint="next"
      description="We send the receipt here."
      status={invalid ? { type: 'error', message: 'Add the part after @, such as example.com.' } : undefined}
      isRequired
      data-testid="email"
    />
  );
}

const magnifier: VectorPath[] = [
  { d: 'M10.5 4a6.5 6.5 0 1 0 0 13a6.5 6.5 0 1 0 0-13z', fill: 'none', stroke: 'currentColor', strokeWidth: 2 },
  { d: 'M15.5 15.5L20 20', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinecap: 'round' },
];

/** Controlled (`value` + `onChange`), with a leading icon and a clear button the app names. */
export function Search() {
  const query = useLocal('query', '');
  return (
    <TextInput
      value={query.value}
      onChange={next => query.set(next)}
      label="Search notes"
      isLabelHidden
      placeholder="Search notes"
      type="search"
      enterKeyHint="search"
      startIcon={<vector aria-hidden paths={magnifier} viewBox={[0, 0, 24, 24]} className="size-4" />}
      hasClear
      clearLabel="Clear search"
      data-testid="search"
    />
  );
}

export function Disabled() {
  const code = useLocal('code', 'LX-2041');
  return <TextInput bind={code} label="Invite code" isDisabled disabledMessage="Codes are fixed once sent." data-testid="code" />;
}
