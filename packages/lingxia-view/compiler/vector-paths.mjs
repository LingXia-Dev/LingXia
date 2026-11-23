import ts from 'typescript';
import { pathSegments, pathSegmentsProblem } from '../src/runtime/vector-path.js';

const at = node => {
  const file = node.getSourceFile();
  const { line, character } = file.getLineAndCharacterOfPosition(node.getStart(file));
  return `${file.fileName}:${line + 1}:${character + 1}`;
};

/** A written-out path `d` (`{ d: '…' }` in an array) over the segment limit, which every render would refuse. */
export function vectorPathProblems(files) {
  const problems = [];
  for (const file of files) {
    const visit = node => {
      if (ts.isPropertyAssignment(node) && node.name.getText(file) === 'd' && ts.isStringLiteralLike(node.initializer)
          && ts.isObjectLiteralExpression(node.parent) && ts.isArrayLiteralExpression(node.parent.parent)) {
        const problem = pathSegmentsProblem(pathSegments(node.initializer.text));
        if (problem) problems.push(`${at(node.initializer)}: ${problem}`);
      }
      ts.forEachChild(node, visit);
    };
    visit(file);
  }
  return problems;
}
