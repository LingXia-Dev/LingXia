import ts from 'typescript';
import { classPieces } from './class-usage.mjs';

// The elements that cannot take a scale or rotation: their native parts (a platform editor, a
// scroll's offset, a slider's thumb under the finger, a web page) are placed in page points,
// unrotated.
const rigid = new Set(['input', 'textarea', 'search', 'scroll', 'slider', 'webview']);

const moment = { starting: ' as it enters', exit: ' as it leaves' };

/** What a rule does to a box's shape, or nothing for translation and the identity. */
function turns(rule) {
  // A slider's thumb scales inside the slider's own drawing.
  if (rule.when?.part) return undefined;
  const { transform, style } = rule;
  if (transform && [transform.scaleX, transform.scaleY].some(value => value !== undefined && value !== 1)) return 'scales';
  if (style?.rotate) return 'rotates';
  for (const track of style?.animation?.transforms ?? []) {
    if (track.kind === 'rotate' && track.angle.some(key => key.value !== 0)) return 'rotates';
    if (track.kind === 'scale' && [...track.x, ...track.y].some(key => key.value !== 1)) return 'scales';
  }
  return undefined;
}

const tagOf = node => (ts.isJsxElement(node) ? node.openingElement : node).tagName;

/** The first rigid element written in `node`'s JSX, itself included; components are opaque. */
function rigidIn(node) {
  let found;
  const visit = child => {
    if (found) return;
    if ((ts.isJsxElement(child) || ts.isJsxSelfClosingElement(child)) && ts.isIdentifier(tagOf(child)) && rigid.has(tagOf(child).text)) {
      found = child;
      return;
    }
    ts.forEachChild(child, visit);
  };
  visit(node);
  return found;
}

const line = node => node.getSourceFile().getLineAndCharacterOfPosition(node.getStart()).line + 1;

const at = node => {
  const file = node.getSourceFile();
  const { line, character } = file.getLineAndCharacterOfPosition(node.getStart(file));
  return `${file.fileName}:${line + 1}:${character + 1}`;
};

/**
 * A box that scales or rotates while it holds an editor, scroll or slider written inside it:
 * at rest, in a state, or by a keyframe animation the page fails when it lands (`errors`); on
 * entering or leaving only its translation moves (`warnings`, for the app's own files).
 */
export function transformedHolderProblems(files, checker, rulesByName, own = () => true) {
  const errors = [], warnings = [];
  for (const file of files) {
    const visit = node => {
      if (ts.isJsxElement(node) || ts.isJsxSelfClosingElement(node)) {
        const opening = ts.isJsxElement(node) ? node.openingElement : node;
        const className = opening.attributes.properties.find(prop => ts.isJsxAttribute(prop) && prop.name.getText(file) === 'className');
        const expression = className?.initializer && (ts.isJsxExpression(className.initializer) ? className.initializer.expression : className.initializer);
        const names = expression ? classPieces(checker, expression).flatMap(({ text }) => text.split(/\s+/)).filter(Boolean) : [];
        for (const name of new Set(names)) {
          const rule = (rulesByName.get(name) ?? []).find(turns);
          if (!rule) continue;
          const held = rigidIn(node);
          if (!held) break;
          const how = turns(rule);
          const element = `${tagOf(held).text === 'input' ? 'an' : 'a'} <${tagOf(held).text}>`;
          const holder = held === node ? element : `a box holding ${element} (line ${line(held)})`;
          const moving = moment[rule.when?.state];
          const web = tagOf(held).text === 'webview';
          if (!moving) errors.push(`${at(className)}: UI_TRANSFORM: ${name} ${how} ${holder}; ${web ? 'a web view' : 'editors, scrolls and sliders'} cannot rotate or scale: transform a sibling, or translate`);
          else if (own(file)) warnings.push(web
            ? `${at(className)}: UI_TRANSFORM: ${name} ${how} ${holder}${moving}; the web page shows as a still picture while it does: fade it, or translate`
            : `${at(className)}: UI_TRANSFORM: ${name} ${how} ${holder}${moving}; only its translation moves: fade it, or translate`);
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(file);
  }
  return { errors, warnings };
}
