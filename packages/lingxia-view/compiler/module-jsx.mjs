import ts from 'typescript';

const code = 'UI_SCOPE';

const at = node => {
  const file = node.getSourceFile();
  const { line, character } = file.getLineAndCharacterOfPosition(node.getStart(file));
  return `${file.fileName}:${line + 1}:${character + 1}`;
};

// Class static initialisers run when the module loads; instance fields and function bodies do not.
const isStatic = node => node.modifiers?.some(modifier => modifier.kind === ts.SyntaxKind.StaticKeyword);
const deferred = node => ts.isFunctionLike(node) || (ts.isPropertyDeclaration(node) && !isStatic(node)) || ts.isTypeNode(node);

/** JSX evaluated when the module loads, which the runtime refuses at page open, as `file:line:col: UI_SCOPE: …`. */
export function moduleJsxProblems(files) {
  const problems = [];
  for (const file of files) {
    const visit = node => {
      if (deferred(node)) return;
      if (ts.isJsxElement(node) || ts.isJsxSelfClosingElement(node) || ts.isJsxFragment(node)) {
        problems.push(`${at(node)}: ${code}: JSX at module scope runs when the page loads, outside render; build it inside the component, or make it a function (\`const row = () => <…/>\`)`);
        return;
      }
      ts.forEachChild(node, visit);
    };
    visit(file);
  }
  return problems;
}
