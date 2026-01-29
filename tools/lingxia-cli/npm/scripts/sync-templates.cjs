const fs = require('fs');
const path = require('path');

const srcDir = path.resolve(__dirname, '../../templates');
const destDir = path.resolve(__dirname, '../templates');

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

if (fs.existsSync(srcDir)) {
  console.log(`Syncing templates from ${srcDir} to ${destDir}...`);
  // Clean destination first to avoid stale files
  if (fs.existsSync(destDir)) {
    fs.rmSync(destDir, { recursive: true, force: true });
  }
  copyDir(srcDir, destDir);
  console.log('Templates synced successfully.');
} else if (fs.existsSync(destDir)) {
  console.log(`Using existing npm templates at ${destDir}.`);
} else {
  console.error(`Templates not found. Expected ${destDir} (or source ${srcDir}).`);
  process.exit(1);
}
