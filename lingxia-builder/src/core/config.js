import fs from "fs";

// Parse app.json configuration
export function parseAppConfig(appConfigPath) {
  try {
    if (!fs.existsSync(appConfigPath)) {
      throw new Error(`App config file not found: ${appConfigPath}`);
    }

    const config = JSON.parse(fs.readFileSync(appConfigPath, "utf-8"));

    if (!config.pages || !Array.isArray(config.pages)) {
      throw new Error(
        'Invalid app.json: "pages" field is required and must be an array',
      );
    }

    if (!config.lxAppId) {
      throw new Error('Invalid app.json: "lxAppId" field is required');
    }

    if (!config.lxAppName) {
      throw new Error('Invalid app.json: "lxAppName" field is required');
    }

    if (!config.version) {
      throw new Error('Invalid app.json: "version" field is required');
    }

    return {
      lxAppId: config.lxAppId,
      lxAppName: config.lxAppName,
      version: config.version,
      pages: config.pages,
    };
  } catch (error) {
    throw new Error(`Failed to parse app config: ${error.message}`);
  }
}
