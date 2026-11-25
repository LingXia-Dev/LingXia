import { type RunChild } from '@lingxia/view';
import { jinshuViewRecipe as r } from '../JinshuView/JinshuView.recipe';
import { tokenizeCodeLine } from './codeTokens';

/** Renders syntax-highlighted `RunChild[]` for a single line of code. */
/** `keyPrefix` must be unique per line: a host flattens nested runs, so token keys meet across lines. */
export function renderHighlightedCodeLine(line: string, language?: string, keyPrefix = 'l'): RunChild {
  if (line === '') return ' ';
  const tokens = tokenizeCodeLine(line, language);
  if (tokens.length === 1 && tokens[0].kind === 'plain') return line;
  return tokens.map((tok, idx) =>
    tok.kind === 'plain' ? (
      tok.text
    ) : (
      <span key={`${keyPrefix}-t${idx}`} className={r.code.token[tok.kind]}>
        {tok.text}
      </span>
    ),
  );
}
