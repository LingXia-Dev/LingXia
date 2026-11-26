import { useLocal } from '@lingxia/view';
import { unused } from '../example';
import { Tokenizer, type TokenizerProps } from './Tokenizer';

const isAddress = (text: string) => (/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(text) ? undefined : `"${text}" is not an email address.`);

/** Who an invitation goes to: type or paste addresses, separated by commas; one that is not an address stays to be fixed. */
export function Recipients() {
  const to = useLocal<readonly string[]>('to', ['mira@harbour.co']);
  return (
    <Tokenizer
      bind={to}
      label="Invite"
      placeholder="name@example.com"
      description="Press Enter or type a comma after each address."
      validate={isAddress}
      data-testid="invite"
    />
  );
}

/** Tags for a post: free words, a comma ends each one. */
export function Tags() {
  const tags = useLocal<readonly string[]>('tags', ['ferries', 'summer']);
  return <Tokenizer bind={tags} label="Tags" placeholder="Add a tag" size="sm" data-testid="tags" />;
}

/** Empty and required, after a failed send. */
export function Empty() {
  const words = useLocal<readonly string[]>('words', []);
  return (
    <Tokenizer
      bind={words}
      label="Keywords"
      placeholder="Words a search should match"
      isRequired
      status={words.value.length === 0 ? { type: 'error', message: 'Add at least one keyword.' } : undefined}
      data-testid="keywords"
    />
  );
}

export function StateDisabled() {
  return <Tokenizer value={['ferries', 'summer']} onChange={unused} label="Tags" isDisabled disabledMessage="Tags are set by the editor." />;
}

/** The home tile. */
export function Specimen() {
  return <Tokenizer value={['ferries']} onChange={unused} label="Tags" isLabelHidden placeholder="Add" className="w-40" />;
}

/** The playground's props that are not plain values. */
export function playground() {
  return { value: ['ferries', 'summer'], removeLabel: (entry: string) => `Remove ${entry}` } satisfies Partial<TokenizerProps>;
}
