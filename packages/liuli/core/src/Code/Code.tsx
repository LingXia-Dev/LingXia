import { cx, type Child } from '@lingxia/view';
import { codeRecipe as r } from './Code.recipe';

export interface CodeProps {
  /** The code: a command, a file name, a value to type exactly. */
  text: string;
  'data-testid'?: string;
}

/** A few words of code inside a sentence. It goes in a `<text>`, among its words. */
export function Code({ text, 'data-testid': testId }: CodeProps) {
  return <span data-testid={testId} className={r.inline.base}>{text}</span>;
}

export interface CodeBlockProps {
  /** The code, with its own line breaks and indentation. */
  code: string;
  /** Names the block for assistive tech: "Shell command", "Example response". */
  label: string;
  /** Shown over the code: a file name, a language. */
  title?: string;
  /** At the end of the title's line: a Button or IconButton that copies the code. The app does the copying. */
  action?: Child;
  /** Lets the code be selected. @default true */
  isSelectable?: boolean;
  className?: string;
  'data-testid'?: string;
}

/** Lines of code as they were written: fixed-width, never wrapped, scrolling sideways. */
export function CodeBlock({ code, label, title, action, isSelectable = true, className, 'data-testid': testId }: CodeBlockProps) {
  return (
    <view role="group" aria-label={label} data-testid={testId} className={cx(r.block.base, className)}>
      {(title !== undefined || action !== undefined) && (
        <view className={r.header.base}>
          <text className={r.title.base}>{title ?? ''}</text>
          {action}
        </view>
      )}
      <scroll className={r.scroll.base}>
        <text className={cx(r.text.base, isSelectable && r.text.selectable)}>{code}</text>
      </scroll>
    </view>
  );
}
