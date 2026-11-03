import { Details, Fragment, Summary, type View } from '@lingxia/view';

type Block = { heading: string; body: string; term: string };

// A definition is two texts with no box of their own: they sit in the definitions grid directly.
function Definition({ block }: { block: Block }) {
  return <><text className="text-sm font-semibold">{block.term}</text><text className="text-sm text-muted">{block.heading}</text></>;
}

export function render({ data }: View<object, { updated: string; blocks: Block[] }>) {
  return (
    <page className="bg-surface">
      <scroll className="grow">
        <column className="p-5 gap-4 max-w-[68ch]">
          <text role="heading" headingLevel={1} className="text-2xl font-bold">Terms of service</text>
          <text className="text-xs text-muted tabular-nums">Updated {data.updated}</text>
          {data.blocks.map((block, index) => (
            <column key={block.heading} className="gap-2">
              <text role="heading" headingLevel={2} className="text-base font-semibold">{index + 1}. {block.heading}</text>
              <text className="text-sm leading-relaxed select-text">
                {block.body} <span className="font-semibold">{block.term}</span> means the service as described in{' '}
                <link onClick={() => {}} className="text-accent underline">section {index + 2}</link>, including{' '}
                <span className="font-mono text-xs bg-surface-sunken">api.example.com</span> and its <span className="italic">successors</span>.
              </text>
              <text className="text-xs text-faint line-clamp-2">{block.body.repeat(3)}</text>
            </column>
          ))}
          <Details defaultOpen>
            <Summary className="py-2 text-base font-semibold">Third-party notices</Summary>
            <text className="text-xs text-muted">This service includes open-source software under the MIT and Apache-2.0 licenses.</text>
          </Details>
          <text role="heading" headingLevel={2} className="text-base font-semibold">Definitions</text>
          <grid className="grid-cols-[auto_1fr] gap-x-4 gap-y-1">
            {data.blocks.map(block => <Fragment key={block.heading}><Definition block={block} /></Fragment>)}
          </grid>
        </column>
      </scroll>
    </page>
  );
}
