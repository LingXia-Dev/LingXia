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

  // Find the Page({...}) call and extract functions from within it
  const pageMatch = code.match(/Page\s*\(\s*\{([\s\S]*)\}\s*\)/);
  if (!pageMatch) {
    console.warn("No Page({}) call found in the code");
    return functions;
  }

  const pageObjectContent = pageMatch[1];

  // Extract object method functions from within Page object
  // Match patterns like: methodName: function() {}, methodName: async function() {}
  const objectMethods = pageObjectContent.match(
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

  // Also extract arrow function methods: methodName: () => {}, methodName: async () => {}
  const arrowMethods = pageObjectContent.match(
    /([a-zA-Z_$][a-zA-Z0-9_$]*)\s*:\s*(?:async\s+)?\([^)]*\)\s*=>\s*\{[\s\S]*?\}/g,
  );

  if (arrowMethods) {
    arrowMethods.forEach((funcCode) => {
      const nameMatch = funcCode.match(
        /([a-zA-Z_$][a-zA-Z0-9_$]*)\s*:\s*(?:async\s+)?\(/,
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
