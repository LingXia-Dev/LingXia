import { useLocal } from '@lingxia/view';
import { Said } from '../example';
import { Button } from '../Button/Button';
import { Select } from '../Select/Select';
import { TextInput } from '../TextInput/TextInput';
import { InputGroup, InputGroupText, type InputGroupProps } from './InputGroup';

/** Fixed text before the field: the scheme is part of the address, never typed. */
export function Website() {
  const site = useLocal('site', '');
  return (
    <InputGroup label="Website" data-testid="website">
      <InputGroupText>https://</InputGroupText>
      <TextInput bind={site} label="Website" placeholder="example.com" type="text" inputMode="text" autoCapitalize="none" data-testid="site" />
    </InputGroup>
  );
}

const currencies = [{ value: 'usd', label: 'USD' }, { value: 'eur', label: 'EUR' }, { value: 'jpy', label: 'JPY' }];

/** A currency and an amount entered as one price. */
export function Price() {
  const currency = useLocal('currency', 'usd');
  const amount = useLocal('amount', '');
  return (
    <InputGroup label="Price" data-testid="price">
      <Select bind={currency} label="Currency" items={currencies} data-testid="currency" />
      <TextInput bind={amount} label="Amount" placeholder="0.00" inputMode="decimal" data-testid="amount" />
    </InputGroup>
  );
}

/** A field and the action it feeds: the button ends the group. */
export function Invite() {
  const email = useLocal('email', '');
  return (
    <Said>
      {say => (
        <InputGroup label="Invite by email" data-testid="invite">
          <TextInput bind={email} label="Email" placeholder="name@example.com" type="email" autoCapitalize="none" data-testid="email" />
          <Button label="Invite" onClick={() => say(email.value ? `Invited ${email.value}` : 'Add an email first')} data-testid="send" />
        </InputGroup>
      )}
    </Said>
  );
}

/** A unit after the value, and a size for the whole group. */
export function Sizes() {
  const [small, large] = [useLocal('small', '72.4'), useLocal('large', '72.4')];
  return (
    <view className="gap-4">
      <InputGroup label="Weight, small" size="sm" data-testid="small">
        <TextInput bind={small} label="Weight" inputMode="decimal" />
        <InputGroupText>kg</InputGroupText>
      </InputGroup>
      <InputGroup label="Weight" data-testid="medium">
        <TextInput bind={large} label="Weight" inputMode="decimal" />
        <InputGroupText>kg</InputGroupText>
      </InputGroup>
    </view>
  );
}

/** A specimen for the gallery. */
export function Specimen() {
  return <view className="w-72"><Price /></view>;
}

export function playground() {
  return {
    children: [
      <InputGroupText key="scheme">https://</InputGroupText>,
      <TextInput key="host" value="" onChange={() => undefined} label="Website" placeholder="example.com" />,
    ],
  } satisfies Partial<InputGroupProps>;
}

