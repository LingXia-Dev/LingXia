import { useLocal } from '@lingxia/view';
import { Markdown } from './Markdown';

const note = `# Trip to the coast

We leave on **Friday at 7:30** and are back *Sunday evening*. The house is
[ten minutes from the beach](https://example.com/house), and the key is in the lockbox.

## To pack

- [x] Tent and pegs
- [x] Stove, with a spare canister
- [ ] Rain jackets
- [ ] Charger for the camera

## On the way

1. Fuel before the motorway
2. Lunch in Setúbal
3. Call ahead if we are later than six
`;

/** A note: headings, emphasis, a link, a task list and a numbered list. */
export function Note() {
  const opened = useLocal('opened', '');
  return (
    <view className="gap-3">
      <Markdown source={note} onLinkClick={opened.set} taskLabels={{ done: 'Done', open: 'Not done' }} data-testid="note" />
      {opened.value !== '' && <text role="status" className="text-footnote text-muted">{`Opened ${opened.value}`}</text>}
    </view>
  );
}

const reference = `Set the token before the first call, then check the reply's \`status\`.

\`\`\`ts
const reply = await fetch('/api/notes', { headers: { authorization: token } });
if (reply.status === 401) await signIn();
\`\`\`

| Status | Meaning | Retry |
|:--|:--|--:|
| 200 | The notes | no |
| 401 | Signed out | once |
| 429 | Too many calls | after 30 s |

> A reply over 512 KiB is streamed. Read it once.
`;

/** Reference text: inline code, a code block that scrolls sideways, a table and a quote. */
export function Reference() {
  return <Markdown source={reference} data-testid="reference" />;
}

/** Small, for a comment or a card. */
export function Comment() {
  return <Markdown size="sm" source={'Looks right to me. Two things:\n\n- the total should use ~~gross~~ **net**\n- see `summary.ts`, line 40'} data-testid="comment" />;
}

/** The home tile. */
export function Specimen() {
  return <Markdown size="sm" source={'**Notes**\n\n- [x] Draft\n- [ ] Review'} />;
}
