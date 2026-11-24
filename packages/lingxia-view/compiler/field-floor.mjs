import ts from 'typescript';
import { field, ignorableFields } from '../src/generated/wire.js';
import { elements, admission, style } from '../src/generated/contract.js';

const ignorable = new Set(ignorableFields);
const has = (object, key) => Object.prototype.hasOwnProperty.call(object, key);
const camel = name => name.replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());

// The oldest host a page asks for: the highest field its JSX props, literal style keys and
// class rules name. It is a lower bound (a prop may reach a field under another name), never
// more than the page writes; the runtime's encoder refuses any field past the host's exactly.
export function fieldFloor(sourceFiles, usages, rulesByName) {
  let floor = 0;
  // A field counts only on an element that takes it, so a prop named like another element's field adds nothing.
  const add = (key, tag) => {
    const allowed = admission.fields[key];
    if (has(field, key) && !ignorable.has(key) && (!allowed || (tag && allowed.includes(elements[tag].wire)))) floor = Math.max(floor, field[key]);
  };
  // A style key may lower to several fields; only the lowest is sure to be written.
  const addStyle = (key, tag) => {
    const known = [].concat((has(style, key) && style[key].wire) || key).filter(name => has(field, name));
    if (known.length) add(known.reduce((low, name) => field[name] < field[low] ? name : low), tag);
  };
  let file;
  const visit = node => {
    if ((ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) && ts.isIdentifier(node.tagName) && has(elements, node.tagName.text)) {
      for (const attribute of node.attributes.properties) {
        if (!ts.isJsxAttribute(attribute)) continue;
        const value = attribute.initializer && ts.isJsxExpression(attribute.initializer) ? attribute.initializer.expression : undefined;
        if (value && ts.isIdentifier(value) && value.text === 'undefined') continue;
        const name = attribute.name.getText(file), tag = node.tagName.text;
        add(camel(name), tag);
        if (name === 'style' && value && ts.isObjectLiteralExpression(value))
          for (const property of value.properties)
            if (ts.isPropertyAssignment(property) && (ts.isIdentifier(property.name) || ts.isStringLiteral(property.name))) addStyle(property.name.text, tag);
      }
    }
    ts.forEachChild(node, visit);
  };
  for (file of sourceFiles) visit(file);
  for (const { value, tag } of usages) {
    if (!has(elements, tag)) continue;
    for (const name of value.split(/\s+/)) for (const rule of (name && rulesByName.get(name)) || []) {
      if (rule.error) continue;
      if (rule.when?.state) add(`${rule.when.state}Style`, tag);
      for (const key of Object.keys(rule.style ?? {})) addStyle(key, tag);
    }
  }
  return floor;
}
