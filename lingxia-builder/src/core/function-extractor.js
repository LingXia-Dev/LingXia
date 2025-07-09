import fs from "fs";

// Extract page functions from JS file
export function extractPageFunctions(jsFilePath) {
  if (!fs.existsSync(jsFilePath)) {
    return [];
  }

  try {
    const code = fs.readFileSync(jsFilePath, "utf-8");
    return parseJavaScriptFunctions(code);
  } catch (error) {
    console.warn(
      `Failed to extract functions from ${jsFilePath}:`,
      error.message,
    );
    return [];
  }
}

function parseJavaScriptFunctions(code) {
  const functions = [];

  // Extract function declarations
  const functionDeclarations = code.match(
    /function\s+([a-zA-Z_$][a-zA-Z0-9_$]*)\s*\([^)]*\)\s*\{[\s\S]*?\}/g,
  );
  if (functionDeclarations) {
    functionDeclarations.forEach((funcCode) => {
      const nameMatch = funcCode.match(/function\s+([a-zA-Z_$][a-zA-Z0-9_$]*)/);
      if (nameMatch) {
        functions.push({
          name: nameMatch[1],
          code: funcCode.trim(),
        });
      }
    });
  }

  // Extract arrow functions
  const arrowFunctions = code.match(
    /(?:const|let|var)\s+([a-zA-Z_$][a-zA-Z0-9_$]*)\s*=\s*\([^)]*\)\s*=>\s*\{[\s\S]*?\}/g,
  );
  if (arrowFunctions) {
    arrowFunctions.forEach((funcCode) => {
      const nameMatch = funcCode.match(
        /(?:const|let|var)\s+([a-zA-Z_$][a-zA-Z0-9_$]*)/,
      );
      if (nameMatch) {
        functions.push({
          name: nameMatch[1],
          code: funcCode.trim(),
        });
      }
    });
  }

  // Extract object method functions (e.g., methodName: function() {} or methodName: async function() {})
  const objectMethods = code.match(
    /([a-zA-Z_$][a-zA-Z0-9_$]*)\s*:\s*(?:async\s+)?function\s*\([^)]*\)\s*\{[\s\S]*?\}/g,
  );
  if (objectMethods) {
    objectMethods.forEach((funcCode) => {
      const nameMatch = funcCode.match(
        /([a-zA-Z_$][a-zA-Z0-9_$]*)\s*:\s*(?:async\s+)?function/,
      );
      if (nameMatch) {
        functions.push({
          name: nameMatch[1],
          code: funcCode.trim(),
        });
      }
    });
  }

  return functions;
}

// Filter functions that should be exposed as page functions
export function filterPageFunctions(functions) {
  // Only exclude actual lifecycle methods and reserved names
  const excludedNames = [
    "onLoad",
    "onReady",
    "onShow",
    "onHide",
    "onUnload",
    "data",
    "Page",
  ];

  return functions.filter((func) => {
    // Exclude lifecycle methods and reserved names
    if (excludedNames.includes(func.name)) {
      return false;
    }

    // Exclude private functions (starting with _)
    if (func.name.startsWith("_")) {
      return false;
    }

    // Only include functions that look like valid identifiers
    if (!/^[a-zA-Z_$][a-zA-Z0-9_$]*$/.test(func.name)) {
      return false;
    }

    return true;
  });
}
