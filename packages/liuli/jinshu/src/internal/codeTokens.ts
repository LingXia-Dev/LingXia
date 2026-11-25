export type CodeTokenKind = 'plain' | 'keyword' | 'string' | 'comment' | 'number' | 'type' | 'attr';

export interface CodeToken {
  readonly kind: CodeTokenKind;
  readonly text: string;
}

const COMMON_KEYWORDS = new Set([
  'import', 'export', 'from', 'default', 'const', 'let', 'var', 'function', 'return',
  'if', 'else', 'for', 'while', 'do', 'switch', 'case', 'break', 'continue', 'try',
  'catch', 'finally', 'throw', 'new', 'typeof', 'instanceof', 'in', 'of', 'async',
  'await', 'yield', 'class', 'extends', 'implements', 'interface', 'type', 'readonly',
  'public', 'private', 'protected', 'static', 'true', 'false', 'null', 'undefined', 'void',
  'fn', 'pub', 'mut', 'impl', 'struct', 'enum', 'trait', 'match', 'use', 'mod', 'crate',
  'where', 'self', 'Self', 'Some', 'None', 'Ok', 'Err', 'def', 'elif', 'lambda', 'pass',
  'with', 'as', 'None', 'True', 'False', 'and', 'or', 'not', 'is', 'select', 'where',
  'insert', 'into', 'update', 'delete', 'join', 'group', 'order', 'by', 'limit',
]);

const IDENT_START_RE = /[A-Za-z_$]/;
const IDENT_PART_RE = /[A-Za-z0-9_$]/;
const DIGIT_RE = /[0-9]/;

/**
 * Zero-dependency, single-line lexical tokenizer for GPU code blocks.
 * Respects LingXia's `<= 256` runs per `<text>` hard cap by merging adjacent plain runs
 * and falling back to plain text past `maxTokens` (default 56).
 */
export function tokenizeCodeLine(line: string, language?: string, maxTokens = 56): CodeToken[] {
  const lang = (language ?? '').toLowerCase();
  if (!line || lang === 'text' || lang === 'plain' || lang === 'txt') {
    return [{ kind: 'plain', text: line }];
  }

  const out: CodeToken[] = [];
  const push = (kind: CodeTokenKind, text: string) => {
    if (!text) return;
    const prev = out[out.length - 1];
    if (prev && prev.kind === kind) {
      out[out.length - 1] = { kind, text: prev.text + text };
    } else {
      out.push({ kind, text });
    }
  };

  const isHashCommentLang =
    lang === 'py' || lang === 'python' || lang === 'sh' || lang === 'bash' || lang === 'zsh' || lang === 'yaml' || lang === 'yml' || lang === 'toml';
  const isMarkupLang = lang === 'html' || lang === 'xml' || lang === 'jinshu' || lang === 'svg';

  let i = 0;
  while (i < line.length) {
    if (out.length >= maxTokens) {
      push('plain', line.slice(i));
      break;
    }

    const ch = line[i];
    const next = line[i + 1];

    // Line comments: //, --, or #
    if ((ch === '/' && next === '/') || (lang === 'sql' && ch === '-' && next === '-') || (isHashCommentLang && ch === '#')) {
      push('comment', line.slice(i));
      break;
    }

    // Block or XML comment on line: /* ... */ or <!-- ... -->
    if (ch === '/' && next === '*') {
      const close = line.indexOf('*/', i + 2);
      if (close >= 0) {
        push('comment', line.slice(i, close + 2));
        i = close + 2;
        continue;
      }
      push('comment', line.slice(i));
      break;
    }
    if (isMarkupLang && line.startsWith('<!--', i)) {
      const close = line.indexOf('-->', i + 4);
      if (close >= 0) {
        push('comment', line.slice(i, close + 3));
        i = close + 3;
        continue;
      }
      push('comment', line.slice(i));
      break;
    }

    // Strings: '...', "...", `...`
    if (ch === '"' || ch === '\'' || ch === '`') {
      let end = i + 1;
      while (end < line.length) {
        if (line[end] === '\\') {
          end += 2;
          continue;
        }
        if (line[end] === ch) {
          end += 1;
          break;
        }
        end += 1;
      }
      push('string', line.slice(i, end));
      i = end;
      continue;
    }

    // HTML / XML / JSX tags: <tag or </tag
    if (isMarkupLang && ch === '<' && (next === '/' || IDENT_START_RE.test(next ?? ''))) {
      let end = i + (next === '/' ? 2 : 1);
      while (end < line.length && /[A-Za-z0-9_:-]/.test(line[end])) end += 1;
      push('keyword', line.slice(i, end));
      i = end;
      continue;
    }

    // Numbers
    if (DIGIT_RE.test(ch) && (i === 0 || !IDENT_PART_RE.test(line[i - 1]))) {
      let end = i + 1;
      while (end < line.length && /[0-9a-fA-FxXoObB_.]/.test(line[end])) end += 1;
      push('number', line.slice(i, end));
      i = end;
      continue;
    }

    // Identifiers & Keywords & PascalCase Types
    if (IDENT_START_RE.test(ch)) {
      let end = i + 1;
      while (end < line.length && IDENT_PART_RE.test(line[end])) end += 1;
      const word = line.slice(i, end);
      if (COMMON_KEYWORDS.has(word) || (lang === 'sql' && COMMON_KEYWORDS.has(word.toLowerCase()))) {
        push('keyword', word);
      } else if (word[0] >= 'A' && word[0] <= 'Z' && /[a-z]/.test(word)) {
        push('type', word);
      } else {
        push('plain', word);
      }
      i = end;
      continue;
    }

    push('plain', ch);
    i += 1;
  }

  return out;
}

