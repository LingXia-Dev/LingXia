import { parse, type ParserOptions } from '@babel/parser';
import { generate } from '@babel/generator';

const AST_PARSE_OPTIONS: ParserOptions = {
  sourceType: 'module',
  plugins: [
    'typescript',
    'jsx',
    'classProperties',
    'decorators-legacy',
    'dynamicImport',
    'objectRestSpread',
    'optionalChaining',
    'nullishCoalescingOperator',
    'topLevelAwait'
  ]
};

export interface InjectPagePathOptions {
  pluginId?: string;
}

export const injectPagePath = (
  logicContent: string,
  pagePath: string,
  options?: InjectPagePathOptions
): string => {
  // Add @plugin/<pluginId>/ prefix for plugin mode
  const finalPath = options?.pluginId
    ? `@plugin/${options.pluginId}/${pagePath}`
    : pagePath;
  const ast = parse(logicContent, AST_PARSE_OPTIONS);
  let modified = false;

  traverseAst(ast.program as BabelNode, node => {
    if (!isPageCall(node)) {
      return;
    }

    const args: BabelNode[] = node.arguments ?? [];
    if (args.length >= 2) {
      return;
    }

    const firstArg = unwrapExpression(args[0]);
    if (!firstArg || firstArg.type !== 'ObjectExpression') {
      return;
    }

    node.arguments = [...args, stringLiteral(finalPath)];
    modified = true;
  });

  if (!modified) {
    return logicContent;
  }

  return generate(ast, { retainLines: true }).code;
};

type BabelNode = {
  type?: string;
  [key: string]: any;
};

const traverseAst = (node: BabelNode | null | undefined, visitor: (node: BabelNode) => void): void => {
  if (!node || typeof node.type !== 'string') {
    return;
  }

  visitor(node);

  for (const value of Object.values(node)) {
    if (!value) continue;

    if (Array.isArray(value)) {
      for (const child of value) {
        if (child && typeof child.type === 'string') {
          traverseAst(child, visitor);
        }
      }
      continue;
    }

    if (value && typeof value.type === 'string') {
      traverseAst(value, visitor);
    }
  }
};

const isPageCall = (node: BabelNode): node is BabelNode => {
  if (node.type !== 'CallExpression') {
    return false;
  }
  const callee = node.callee as BabelNode | undefined;
  return Boolean(callee && callee.type === 'Identifier' && callee.name === 'Page');
};

const unwrapExpression = (node?: BabelNode | null): BabelNode | null => {
  let current: BabelNode | null = node ?? null;

  while (current) {
    if (
      current.type === 'TSAsExpression' ||
      current.type === 'TSTypeAssertion' ||
      current.type === 'TSNonNullExpression' ||
      current.type === 'TypeCastExpression'
    ) {
      current = current.expression as BabelNode;
      continue;
    }

    if (current.type === 'ParenthesizedExpression') {
      current = current.expression as BabelNode;
      continue;
    }

    break;
  }

  return current;
};

const stringLiteral = (value: string): BabelNode => ({
  type: 'StringLiteral',
  value
});
