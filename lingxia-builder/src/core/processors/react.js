import fs from "fs";
import path from "path";

/**
 * React template processor
 */
export class ReactProcessor {
  constructor() {
    this.templateDir = path.resolve(
      import.meta.dirname,
      "../../templates/react",
    );
  }

  async process(buildDir, functions, pageFiles, generateFunctionScript) {
    // Copy template files
    const templateFiles = ["package.json", "vite.config.js", "index.html"];
    this.copyFiles(templateFiles, buildDir);

    // Generate main.jsx with function injection
    const mainJsxTemplate = fs.readFileSync(
      path.join(this.templateDir, "main.jsx"),
      "utf-8",
    );

    const functionNames = functions.map((f) => f.name);
    const functionInjection = generateFunctionScript(functionNames);
    const mainJsxContent = mainJsxTemplate.replace(
      /\/\*\s*\{\{PAGE_FUNCTIONS\}\}\s*\*\//,
      functionInjection,
    );

    fs.writeFileSync(path.join(buildDir, "main.jsx"), mainJsxContent);

    // Copy user's React component with correct extension
    if (pageFiles.main.exists) {
      const userFileExt = path.extname(pageFiles.main.path);
      const destFile = path.join(buildDir, `App${userFileExt}`);
      fs.copyFileSync(pageFiles.main.path, destFile);

      // Update main.jsx import to use correct extension
      const mainJsxPath = path.join(buildDir, "main.jsx");
      let updatedContent = fs.readFileSync(mainJsxPath, "utf-8");
      updatedContent = updatedContent.replace(
        "import App from './App.jsx'",
        `import App from './App${userFileExt}'`,
      );
      fs.writeFileSync(mainJsxPath, updatedContent);
    }
  }

  copyFiles(files, buildDir) {
    files.forEach((file) => {
      const srcPath = path.join(this.templateDir, file);
      const destPath = path.join(buildDir, file);
      if (fs.existsSync(srcPath)) {
        fs.copyFileSync(srcPath, destPath);
      }
    });
  }
}
