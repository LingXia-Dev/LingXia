import ts from 'typescript';

// What makes each control act; without one it is enabled and focusable but does nothing.
const acting = {
  button: ['onClick', 'popoverTarget', 'interestFor', 'drag'],
  checkbox: ['onChange', 'bind'],
  switch: ['onChange', 'bind'],
  radio: ['onChange', 'bind'],
  select: ['onChange', 'bind'],
  slider: ['onChange', 'bind'],
};

const code = 'UI_INERT';

const at = node => {
  const file = node.getSourceFile();
  const { line, character } = file.getLineAndCharacterOfPosition(node.getStart(file));
  return `${file.fileName}:${line + 1}:${character + 1}`;
};

/** Controls written with no handler, no `bind` and no `disabled`, as `file:line:col: UI_INERT: …`. */
export function inertControlWarnings(files) {
  const warnings = [];
  for (const file of files) {
    const visit = node => {
      if ((ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) && ts.isIdentifier(node.tagName)) {
        const tag = node.tagName.text;
        const props = node.attributes.properties;
        // A spread may carry the handler.
        if (acting[tag] && !props.some(ts.isJsxSpreadAttribute)) {
          const names = new Set(props.map(prop => prop.name.getText(file)));
          // A menu item closes its menu.
          const menuItem = props.some(prop => prop.name.getText(file) === 'role' && prop.initializer && ts.isStringLiteral(prop.initializer) && prop.initializer.text === 'menuitem');
          // A button that hears the pointer and reports a value is stepped, not clicked.
          const stepped = tag === 'button' && names.has('onPointer') && names.has('aria-valuenow');
          if (!names.has('disabled') && !menuItem && !stepped && !acting[tag].some(name => names.has(name))) {
            const add = tag === 'button' ? 'onClick' : 'onChange or bind';
            warnings.push(`${at(node)}: ${code}: this <${tag}> does nothing: add ${add}, or mark it disabled`);
          }
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(file);
  }
  return warnings;
}
