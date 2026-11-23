import { useLocal } from '@lingxia/view';
import { Held, unused } from '../example';
import { Button } from '../Button/Button';
import { IconButton } from '../IconButton/IconButton';
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

/** Controlled (`value` + `onChange`), as a search field with a clear button the app names. */
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

/** The home tile. */
export function Specimen() {
  return <TextInput value="Ana Lima" onChange={unused} label="Name" isLabelHidden size="sm" className="w-28" />;
}

export function Plain() {
  const name = useLocal('name', '');
  return <TextInput bind={name} label="Full name" placeholder="As it appears on your card" autoComplete="name" />;
}

export function Sizes() {
  const query = useLocal('query', '');
  return (
    <view className="gap-3">
      <TextInput bind={query} label="Small" size="sm" placeholder="Filter" />
      <TextInput bind={query} label="Medium" placeholder="Filter" />
      <TextInput bind={query} label="Large" size="lg" placeholder="Filter" />
    </view>
  );
}

export function StateEmpty() {
  return <Held value="">{(city, set) => <TextInput value={city} onChange={set} label="City" placeholder="Lisbon" />}</Held>;
}

export function StateFilled() {
  return <Held value="Lisbon">{(city, set) => <TextInput value={city} onChange={set} label="City" />}</Held>;
}

export function StateInvalid() {
  return <Held value="ana@">{(email, set) => <TextInput value={email} onChange={set} label="Work email" status={/@./.test(email) ? undefined : { type: 'error', message: 'Add the domain.' }} />}</Held>;
}

export function StateDisabled() {
  return <TextInput value="LX-2041" onChange={unused} label="Code" isDisabled />;
}

/** Fixed text beside the value says its unit or its start, so the person types only what changes. */
export function Adornments() {
  const price = useLocal('price', '24.00');
  const site = useLocal('site', '');
  const weight = useLocal('weight', '');
  return (
    <view className="gap-4">
      <TextInput label="Price" bind={price} prefix="$" suffix="USD" inputMode="decimal" data-testid="price" />
      <TextInput label="Website" bind={site} prefix="https://" placeholder="example.com" autoCapitalize="none" data-testid="site" />
      <TextInput label="Weight" bind={weight} suffix="kg" inputMode="decimal" optionalLabel="optional" data-testid="weight" />
    </view>
  );
}

/** The app is checking what was typed; the field stays editable. */
export function Checking() {
  const handle = useLocal('handle', 'ana');
  return <TextInput bind={handle} label="Username" prefix="@" description="Checking it is free." isLoading autoCapitalize="none" data-testid="handle" />;
}

/** A value to read and copy, not to change. */
export function ReadOnly() {
  return <TextInput value="C-204188" onChange={unused} label="Customer ID" description="Set when the account was opened." isReadOnly data-testid="iban" />;
}

/** Signing in: the password hides what is typed until its button shows it, and a failed sign-in moves focus back to it. */
export function SignIn() {
  const email = useLocal('email', 'ana@example.com');
  const password = useLocal('password', '');
  const checked = useLocal('checked', '');
  const failed = useLocal('failed', false);
  const signIn = () => {
    const ok = password.value.length >= 8;
    failed.set(!ok);
    if (!ok) password.focus();
  };
  return (
    <view className="gap-4">
      <TextInput
        bind={email}
        label="Email"
        type="email"
        autoComplete="email"
        autoCapitalize="none"
        enterKeyHint="next"
        onBlur={() => checked.set(email.value)}
        status={checked.value !== '' && !checked.value.includes('@') ? { type: 'error', message: 'Add the part after @, such as example.com.' } : undefined}
        data-testid="sign-in-email"
      />
      <TextInput
        bind={password}
        label="Password"
        type="password"
        autoComplete="current-password"
        enterKeyHint="go"
        onSubmit={signIn}
        status={failed.value ? { type: 'error', message: 'Passwords here are at least 8 characters.' } : undefined}
        data-testid="password"
      />
      <Button label="Sign in" variant="primary" width="content" onClick={signIn} data-testid="sign-in" />
    </view>
  );
}

/** A link to share: read-only, with a copy button inside the box. */
export function ShareLink() {
  const copied = useLocal('copied', false);
  return (
    <TextInput
      value="https://notes.example.com/s/q8Hk2"
      onChange={unused}
      label="Link"
      description={copied.value ? 'Copied.' : 'Anyone with the link can read the note.'}
      isReadOnly
      endContent={<IconButton icon="copy" label="Copy link" size="sm" variant="ghost" onClick={() => copied.set(true)} data-testid="copy-link" />}
      data-testid="share-link"
    />
  );
}

/** On a right-to-left page each value keeps its own order: the address by its first letter, the email and phone number left to right, and a MAC address because the app says so. */
export function RightToLeft() {
  return (
    <view dir="rtl" className="gap-4" data-testid="rtl-fields">
      <TextInput value="2847 Fillmore St, San Francisco" onChange={unused} label="כתובת" data-testid="rtl-address" />
      <TextInput value="רחוב הרצל 12, תל אביב" onChange={unused} label="כתובת למשלוח" data-testid="rtl-address-he" />
      <TextInput value="dana@example.com" onChange={unused} label="דוא״ל" type="email" data-testid="rtl-email" />
      <TextInput value="+1 (415) 555-0132" onChange={unused} label="טלפון" type="tel" data-testid="rtl-phone" />
      <TextInput value="A4:C1:38:0F:22:9B" onChange={unused} label="כתובת MAC" dir="ltr" data-testid="rtl-mac" />
    </view>
  );
}
