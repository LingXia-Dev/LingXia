import { Button } from '@liulidesign/core/Button';
import { Popover } from '@liulidesign/core/Popover';
import { createDocument, heading, paragraph } from '../schema/helpers';
import { JinshuView } from '../JinshuView/JinshuView';
import { JinshuOutline, useJinshuNav } from './JinshuOutline';

const doc = createDocument({
  blocks: [
    heading(1, 'Limits', 'h1'),
    paragraph('Where a function heads.', 'p1'),
    heading(2, 'Definition', 'h2'),
    heading(2, 'Exercises', 'h3'),
    heading(3, '', 'h4'),
  ],
});

const guide = createDocument({
  title: 'Onboarding guide',
  blocks: [
    heading(2, 'Your first day', 'g1'),
    paragraph('Collect your badge at reception and set up your laptop with IT.', 'g1p'),
    heading(2, 'Your first week', 'g2'),
    paragraph('Meet your team, read the handbook and pick a starter task.', 'g2p'),
    heading(3, 'Accounts to request', 'g3'),
    paragraph('Email, chat, the code host and the design library.', 'g3p'),
    heading(2, 'Your first month', 'g4'),
    paragraph('Ship your starter task and book a check-in with your manager.', 'g4p'),
  ],
});

/** Headings by level, with each section's progress; the one being read is tinted. */
export function Progress() {
  const nav = useJinshuNav('outline-example');
  return <JinshuOutline doc={doc} nav={nav} status={{ h1: 'done', h2: 'current', h3: 'locked' }} data-testid="outline" />;
}

/** In a side column beside the document, sharing one nav handle: pressing a heading scrolls the reader to it. */
export function BesideTheDocument() {
  const nav = useJinshuNav('outline-beside');
  return (
    <view className="flex-row items-start gap-6">
      <JinshuOutline doc={guide} nav={nav} className="w-48 shrink-0" />
      <JinshuView doc={guide} nav={nav} className="grow min-w-0" />
    </view>
  );
}

/** On a phone: the outline in a popover from a Contents button, over the document. */
export function InPopover() {
  const nav = useJinshuNav('outline-popover');
  return (
    <view className="gap-3">
      <Popover label="Contents" content={<JinshuOutline doc={guide} nav={nav} title="On this page" />}>
        <Button label="Contents" startIcon="list" variant="secondary" size="sm" />
      </Popover>
      <JinshuView doc={guide} nav={nav} />
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return <JinshuOutline doc={doc} status={{ h1: 'done', h2: 'current' }} className="w-40" />;
}
