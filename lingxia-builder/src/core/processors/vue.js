import fs from "fs";
import path from "path";

/**
 * Vue template processor
 */
export class VueProcessor {
  constructor() {
    this.templateDir = path.resolve(import.meta.dirname, "../../templates/vue");
  }

  async process(buildDir, functions, pageFiles, generateFunctionScript) {
    // Copy template files
    const templateFiles = ["package.json", "vite.config.js", "index.html"];
    this.copyFiles(templateFiles, buildDir);

    // Generate main.js with function injection
    const mainJsTemplate = fs.readFileSync(
      path.join(this.templateDir, "main.js"),
      "utf-8",
    );

    const functionNames = functions.map((f) => f.name);
    const functionInjection = generateFunctionScript(functionNames);
    const mainJsContent = mainJsTemplate.replace(
      /\/\*\s*\{\{PAGE_FUNCTIONS\}\}\s*\*\//,
      functionInjection,
    );

    fs.writeFileSync(path.join(buildDir, "main.js"), mainJsContent);

    // Copy user's Vue component as App.vue
    if (pageFiles.main.exists) {
      const destFile = path.join(buildDir, "App.vue");
      fs.copyFileSync(pageFiles.main.path, destFile);
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
