import { useLocal } from '@lingxia/view';
import { IconButton } from '../IconButton/IconButton';
import { Code, CodeBlock } from './Code';

const install = `npm install @liulidesign/core
npx lingxia dev --platform ios`;

const response = `{
  "id": "note_8f2c",
  "title": "Trip to the coast",
  "updatedAt": "2026-11-14T09:42:00Z",
  "sharedWith": ["ana@example.com", "kai@example.com"]
}`;

/** A command to copy: the title says what it is, and the app copies it. */
export function Command() {
  const copied = useLocal('copied', false);
  return (
    <view className="gap-2">
      <CodeBlock
        code={install}
        label="Install commands"
        title="Terminal"
        action={<IconButton label="Copy the commands" icon="copy" size="sm" variant="ghost" onClick={() => copied.set(true)} data-testid="copy" />}
        data-testid="install"
      />
      {copied.value && <text role="status" className="text-footnote text-muted">Copied</text>}
    </view>
  );
}

/** Without a header: an example response whose long line scrolls sideways. */
export function Plain() {
  return <CodeBlock code={response} label="Example response" data-testid="response" />;
}

/** In a sentence: a file name and a value to type exactly. */
export function InSentence() {
  return (
    <text className="text-body text-text">
      Set <Code text="minRuntime" data-testid="key" /> in <Code text="lxapp.json" /> to the oldest runtime you test on.
    </text>
  );
}

/** The home tile. */
export function Specimen() {
  return <CodeBlock code={'const a = 1;\nconst b = 2;'} label="Sample" className="w-36" />;
}
