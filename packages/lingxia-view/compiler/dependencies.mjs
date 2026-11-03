// Resolves the package's own dependencies for tools that live outside it (contracts/view).
export const resolveDependency = specifier => import.meta.resolve(specifier);
export const loadDependency = specifier => import(specifier);
