#!/usr/bin/env node
// Build a standalone binary using esbuild (bundle) + Node.js SEA (Single Executable Application)
// Everything (JS + native pty.node + web assets) is embedded into a single executable.

import { execSync } from 'node:child_process';
import { writeFileSync, readFileSync, copyFileSync, mkdirSync, existsSync } from 'node:fs';
import { join } from 'node:path';

mkdirSync('release', { recursive: true });

// Step 1: Bundle with esbuild into single CJS file
console.log('Step 1: Bundling with esbuild...');
execSync(
  'npx esbuild src/cli.ts --bundle --platform=node --format=cjs ' +
  '--outfile=dist/rtb-bundle.cjs ' +
  '--define:import.meta.url=__import_meta_url',
  { stdio: 'inherit' }
);

// Prepend banner: shims for SEA environment
const banner = `
var __import_meta_url = typeof document === "undefined" ? require("url").pathToFileURL(__filename || process.execPath).href : "";
if (require("node:module").isBuiltin === void 0 || require("node:sea") !== void 0) {
  var _origRequire = require;
  require = require("node:module").createRequire(process.execPath);
  require.main = _origRequire.main;
}
`.trim();
let bundle = readFileSync('dist/rtb-bundle.cjs', 'utf-8');

// Patch node-pty's loadNativeModule: extract pty.node + spawn-helper from SEA asset at runtime
const oldLoader = 'function loadNativeModule(name) {';
const newLoader = `function loadNativeModule(name) {
      // SEA: extract embedded native files to cache dir, then dlopen
      try {
        var sea = require("node:sea");
        if (sea.isSea()) {
          var fs = require("fs"), path = require("path"), os = require("os");
          var cacheDir = path.join(os.homedir(), ".rtb", "native", process.platform + "-" + process.arch);
          fs.mkdirSync(cacheDir, { recursive: true });
          // Extract pty.node
          var cached = path.join(cacheDir, name + ".node");
          fs.writeFileSync(cached, Buffer.from(sea.getRawAsset(name + ".node")));
          // Extract spawn-helper (required by node-pty on unix)
          try {
            var helperPath = path.join(cacheDir, "spawn-helper");
            fs.writeFileSync(helperPath, Buffer.from(sea.getRawAsset("spawn-helper")));
            fs.chmodSync(helperPath, 0o755);
          } catch(_h) {}
          return { dir: cacheDir + "/", module: require(cached) };
        }
      } catch(_e) {}
      // Non-SEA fallback (dev mode):`;
if (!bundle.includes(oldLoader)) {
  throw new Error('Could not find loadNativeModule in bundle — node-pty API may have changed');
}
bundle = bundle.replace(oldLoader, newLoader);

writeFileSync('dist/rtb-bundle.cjs', banner + '\n' + bundle);

// Step 2: Locate platform-specific pty.node for embedding
const nodePtyDir = 'node_modules/node-pty';
const prebuildPty = join(nodePtyDir, 'prebuilds', `${process.platform}-${process.arch}`, 'pty.node');
const buildReleasePty = join(nodePtyDir, 'build', 'Release', 'pty.node');
let ptyNodePath;
if (existsSync(prebuildPty)) {
  ptyNodePath = prebuildPty;
} else if (existsSync(buildReleasePty)) {
  ptyNodePath = buildReleasePty;
} else {
  throw new Error('pty.node not found — cannot build standalone binary');
}
console.log(`Using pty.node from: ${ptyNodePath}`);

// Also locate spawn-helper (required by node-pty on unix)
const spawnHelperPath = ptyNodePath.replace('pty.node', 'spawn-helper');
const buildSpawnHelper = join(nodePtyDir, 'build', 'Release', 'spawn-helper');
const spawnHelper = existsSync(spawnHelperPath) ? spawnHelperPath
  : existsSync(buildSpawnHelper) ? buildSpawnHelper : null;
if (spawnHelper) {
  console.log(`Using spawn-helper from: ${spawnHelper}`);
} else {
  console.warn('Warning: spawn-helper not found — PTY may not work on this platform');
}

// Step 3: Create SEA config with native files + web assets embedded
console.log('Step 2: Creating SEA blob...');
const seaConfig = {
  main: 'dist/rtb-bundle.cjs',
  output: 'dist/sea-prep.blob',
  disableExperimentalSEAWarning: true,
  useSnapshot: false,
  useCodeCache: true,
  assets: {
    'pty.node': ptyNodePath,
    ...(spawnHelper ? { 'spawn-helper': spawnHelper } : {}),
  },
};
const webFiles = ['web/index.html', 'web/commands.json', 'web/sw.js'];
for (const f of webFiles) {
  if (existsSync(f)) seaConfig.assets[f] = f;
}

writeFileSync('dist/sea-config.json', JSON.stringify(seaConfig, null, 2));
execSync('node --experimental-sea-config dist/sea-config.json', { stdio: 'inherit' });

// Step 4: Create the executable
console.log('Step 3: Creating executable...');
const nodeBin = process.execPath;
const outputBin = 'release/rtb';
copyFileSync(nodeBin, outputBin);

if (process.platform === 'darwin') {
  execSync(`codesign --remove-signature ${outputBin}`, { stdio: 'inherit' });
}

execSync(
  `npx postject ${outputBin} NODE_SEA_BLOB dist/sea-prep.blob ` +
  '--sentinel-fuse NODE_SEA_FUSE_fce680ab2cc467b6e072b8b5df1996b2 ' +
  (process.platform === 'darwin' ? '--macho-segment-name NODE_SEA ' : ''),
  { stdio: 'inherit' }
);

if (process.platform === 'darwin') {
  execSync(`codesign --sign - --entitlements entitlements.plist --force ${outputBin}`, { stdio: 'inherit' });
}

// Clean up stale release artifacts
for (const f of ['release/pty.node', 'release/node_modules']) {
  execSync(`rm -rf ${f}`, { stdio: 'inherit' });
}

console.log('Done! Single binary at release/rtb');
