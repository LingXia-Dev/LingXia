const fs = require('fs');
const path = require('path');

const srcDir = path.resolve(__dirname, '../../templates/lxapp-create');
const destDir = path.resolve(__dirname, '../templates/lxapp-create');

function copyDir(src, dest) {
  fs.mkdirSync(dest, { recursive: true });
  const entries = fs.readdirSync(src, { withFileTypes: true });

  for (const entry of entries) {
    const srcPath = path.join(src, entry.name);
    const destPath = path.join(dest, entry.name);

    if (entry.isDirectory()) {
      copyDir(srcPath, destPath);
    } else {
      fs.copyFileSync(srcPath, destPath);
    }
  }
}

console.log(`Syncing templates from ${srcDir} to ${destDir}...`);

if (fs.existsSync(srcDir)) {
  // Clean destination first to avoid stale files
  if (fs.existsSync(destDir)) {
    fs.rmSync(destDir, { recursive: true, force: true });
  }
  copyDir(srcDir, destDir);
  console.log('Templates synced successfully.');
} else {
  console.error(`Source templates directory not found: ${srcDir}`);
  process.exit(1);
}
