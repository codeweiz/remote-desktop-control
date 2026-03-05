# CI/CD + Cross-Platform Binary Packaging + QR Code Connect Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable cross-platform binary releases via GitHub Actions and add QR code scanning for mobile app server connection.

**Architecture:** Use @yao-pkg/pkg to bundle Node.js + native modules + web assets into standalone binaries for 4 platforms (linux-x64, linux-arm64, darwin-x64, darwin-arm64). GitHub Actions CI runs tests on push/PR; release workflow builds binaries on tag push or manual trigger. Server prints QR code at startup; mobile app scans it to auto-connect.

**Tech Stack:** @yao-pkg/pkg, GitHub Actions (matrix build), qrcode-terminal (server-side QR), expo-camera (mobile QR scanner)

---

### Task 1: Add QR Code Output to Server Startup

**Files:**
- Modify: `package.json` (add `qrcode-terminal` dependency)
- Modify: `src/server.ts:273-298` (add QR code output after printing URLs)

**Step 1: Install qrcode-terminal**

Run: `npm install qrcode-terminal`

**Step 2: Add QR code generation to server startup**

In `src/server.ts`, add import at top:

```typescript
import QRCode from 'qrcode-terminal';
```

Replace the "Print access info" block (lines 273-298) with:

```typescript
  // Print access info
  const localIP = getLocalIP();
  const localUrl = `http://${localIP}:${config.port}?token=${token}`;
  console.log('');
  console.log('Remote Terminal Bridge v2 started!');
  console.log(`  Web Panel:    ${localUrl}`);
  console.log(`  Local:        http://localhost:${config.port}?token=${token}`);

  let tunnelUrl: string | null = null;
  if (config.tunnel) {
    try {
      const tunnelConfig = getTunnelConfig();
      const result = await startTunnel(
        tunnelConfig
          ? { port: config.port, namedTunnel: tunnelConfig.name, hostname: tunnelConfig.hostname }
          : { port: config.port }
      );
      tunnelUrl = `${result.url}?token=${token}`;
      console.log(`  Tunnel:       ${tunnelUrl}`);
    } catch (err) {
      console.error(`  Tunnel:       ${(err as Error).message}`);
    }
  }
  if (config.feishu) {
    console.log(`  Feishu:       connected (long connection)`);
  }

  // QR code for mobile app connection
  const qrUrl = tunnelUrl || localUrl;
  const mobileLink = `rtb://connect?address=${encodeURIComponent(
    tunnelUrl ? new URL(tunnelUrl).host : `${localIP}:${config.port}`
  )}&token=${encodeURIComponent(token)}&ssl=${tunnelUrl ? '1' : '0'}`;
  console.log('');
  console.log('  Mobile: scan QR code to connect');
  QRCode.generate(mobileLink, { small: true }, (code: string) => {
    for (const line of code.split('\n')) {
      console.log(`  ${line}`);
    }
    console.log('');
  });
```

**Step 3: Add type declaration for qrcode-terminal**

`qrcode-terminal` has no @types package. Add a declaration file `src/qrcode-terminal.d.ts`:

```typescript
declare module 'qrcode-terminal' {
  interface Options {
    small?: boolean;
  }
  function generate(text: string, opts?: Options, cb?: (code: string) => void): void;
  export default { generate };
}
```

**Step 4: Build and test manually**

Run: `npm run build && node dist/cli.js start`
Expected: Server starts, QR code is printed in terminal. QR encodes `rtb://connect?address=...&token=...&ssl=...`.

**Step 5: Commit**

```bash
git add package.json package-lock.json src/server.ts src/qrcode-terminal.d.ts
git commit -m "feat: print QR code at server startup for mobile connection"
```

---

### Task 2: Add QR Code Scanner to Mobile Connect Screen

**Files:**
- Modify: `mobile/package.json` (add `expo-camera` dependency)
- Modify: `mobile/app.json` (add `expo-camera` plugin)
- Modify: `mobile/app/connect.tsx` (add scan button + camera modal)

**Step 1: Install expo-camera**

Run: `cd mobile && npx expo install expo-camera`

**Step 2: Add expo-camera plugin to app.json**

In `mobile/app.json`, update the `plugins` array:

```json
"plugins": [
  "expo-notifications",
  "expo-router",
  [
    "expo-camera",
    {
      "cameraPermission": "Allow RTB to use camera for QR code scanning"
    }
  ]
]
```

**Step 3: Add QR scanner to connect screen**

Replace `mobile/app/connect.tsx` with:

```tsx
import { useState } from 'react';
import { View, Text, TextInput, TouchableOpacity, StyleSheet, Alert, Modal } from 'react-native';
import { useRouter } from 'expo-router';
import { CameraView, useCameraPermissions } from 'expo-camera';
import { useServer } from '../hooks/useServer';

export default function ConnectScreen() {
  const router = useRouter();
  const { save } = useServer();
  const [address, setAddress] = useState('');
  const [token, setToken] = useState('');
  const [useSSL, setUseSSL] = useState(true);
  const [scanning, setScanning] = useState(false);
  const [permission, requestPermission] = useCameraPermissions();

  function parseRtbUrl(url: string): { address: string; token: string; ssl: boolean } | null {
    try {
      // rtb://connect?address=...&token=...&ssl=0|1
      const match = url.match(/^rtb:\/\/connect\?(.+)$/);
      if (!match) return null;
      const params = new URLSearchParams(match[1]);
      const addr = params.get('address');
      const tok = params.get('token');
      if (!addr || !tok) return null;
      return { address: addr, token: tok, ssl: params.get('ssl') === '1' };
    } catch {
      return null;
    }
  }

  async function handleConnect(addr?: string, tok?: string, ssl?: boolean) {
    const finalAddress = addr || address.trim();
    const finalToken = tok || token.trim();
    const finalSSL = ssl !== undefined ? ssl : useSSL;

    if (!finalAddress || !finalToken) {
      Alert.alert('Error', 'Please enter server address and token');
      return;
    }

    const config = { address: finalAddress, token: finalToken, useSSL: finalSSL };

    try {
      const base = `${finalSSL ? 'https' : 'http'}://${config.address}`;
      const res = await fetch(`${base}/api/sessions`);
      if (!res.ok) throw new Error('Connection failed');
    } catch {
      Alert.alert('Connection Failed', 'Could not reach the server. Check address and try again.');
      return;
    }

    await save(config);
    router.replace('/(tabs)/sessions');
  }

  async function handleScanPress() {
    if (!permission?.granted) {
      const result = await requestPermission();
      if (!result.granted) {
        Alert.alert('Permission Required', 'Camera permission is needed to scan QR codes.');
        return;
      }
    }
    setScanning(true);
  }

  function handleBarCodeScanned({ data }: { data: string }) {
    setScanning(false);
    const parsed = parseRtbUrl(data);
    if (!parsed) {
      Alert.alert('Invalid QR Code', 'This QR code is not a valid RTB connection code.');
      return;
    }
    // Auto-fill and connect
    setAddress(parsed.address);
    setToken(parsed.token);
    setUseSSL(parsed.ssl);
    handleConnect(parsed.address, parsed.token, parsed.ssl);
  }

  return (
    <View style={styles.container}>
      <Text style={styles.title}>RTB</Text>
      <Text style={styles.subtitle}>Remote Terminal Bridge</Text>

      <View style={styles.form}>
        <TouchableOpacity style={styles.scanBtn} onPress={handleScanPress}>
          <Text style={styles.scanText}>Scan QR Code</Text>
        </TouchableOpacity>

        <View style={styles.dividerRow}>
          <View style={styles.dividerLine} />
          <Text style={styles.dividerText}>or enter manually</Text>
          <View style={styles.dividerLine} />
        </View>

        <Text style={styles.label}>Server Address</Text>
        <TextInput
          style={styles.input}
          placeholder="rtb.micro-boat.com"
          placeholderTextColor="#484f58"
          value={address}
          onChangeText={setAddress}
          autoCapitalize="none"
          autoCorrect={false}
        />

        <Text style={styles.label}>Token</Text>
        <TextInput
          style={styles.input}
          placeholder="Paste token from server output"
          placeholderTextColor="#484f58"
          value={token}
          onChangeText={setToken}
          autoCapitalize="none"
          autoCorrect={false}
          secureTextEntry
        />

        <View style={styles.sslRow}>
          <Text style={styles.label}>Use HTTPS</Text>
          <TouchableOpacity
            style={[styles.toggle, useSSL && styles.toggleActive]}
            onPress={() => setUseSSL(!useSSL)}
          >
            <Text style={styles.toggleText}>{useSSL ? 'ON' : 'OFF'}</Text>
          </TouchableOpacity>
        </View>

        <TouchableOpacity style={styles.connectBtn} onPress={() => handleConnect()}>
          <Text style={styles.connectText}>Connect</Text>
        </TouchableOpacity>
      </View>

      <Modal visible={scanning} animationType="slide">
        <View style={styles.cameraContainer}>
          <CameraView
            style={StyleSheet.absoluteFill}
            barcodeScannerSettings={{ barcodeTypes: ['qr'] }}
            onBarcodeScanned={handleBarCodeScanned}
          />
          <View style={styles.cameraOverlay}>
            <Text style={styles.cameraTitle}>Scan RTB QR Code</Text>
            <Text style={styles.cameraHint}>Point camera at the QR code in your terminal</Text>
          </View>
          <TouchableOpacity style={styles.cancelBtn} onPress={() => setScanning(false)}>
            <Text style={styles.cancelText}>Cancel</Text>
          </TouchableOpacity>
        </View>
      </Modal>
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: '#0d1117', justifyContent: 'center', padding: 32 },
  title: { fontSize: 36, fontWeight: '800', color: '#f0f6fc', textAlign: 'center' },
  subtitle: { fontSize: 14, color: '#8b949e', textAlign: 'center', marginBottom: 40 },
  form: { gap: 12 },
  label: { fontSize: 13, color: '#8b949e', fontWeight: '500', marginBottom: 4 },
  input: {
    backgroundColor: '#161b22', borderWidth: 1, borderColor: '#30363d',
    borderRadius: 8, padding: 12, color: '#c9d1d9', fontSize: 15, marginBottom: 8,
  },
  sslRow: { flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', marginVertical: 8 },
  toggle: {
    paddingHorizontal: 16, paddingVertical: 6, borderRadius: 12,
    backgroundColor: '#21262d',
  },
  toggleActive: { backgroundColor: '#238636' },
  toggleText: { color: '#c9d1d9', fontSize: 13, fontWeight: '600' },
  scanBtn: {
    backgroundColor: '#1f6feb', padding: 14, borderRadius: 8,
    alignItems: 'center',
  },
  scanText: { color: '#fff', fontSize: 16, fontWeight: '600' },
  dividerRow: { flexDirection: 'row', alignItems: 'center', marginVertical: 12 },
  dividerLine: { flex: 1, height: 1, backgroundColor: '#30363d' },
  dividerText: { color: '#484f58', fontSize: 12, marginHorizontal: 12 },
  connectBtn: {
    backgroundColor: '#238636', padding: 14, borderRadius: 8,
    alignItems: 'center', marginTop: 16,
  },
  connectText: { color: '#fff', fontSize: 16, fontWeight: '600' },
  cameraContainer: { flex: 1, backgroundColor: '#000' },
  cameraOverlay: {
    position: 'absolute', top: 80, left: 0, right: 0, alignItems: 'center',
  },
  cameraTitle: { color: '#fff', fontSize: 20, fontWeight: '700' },
  cameraHint: { color: '#aaa', fontSize: 14, marginTop: 8 },
  cancelBtn: {
    position: 'absolute', bottom: 60, alignSelf: 'center',
    backgroundColor: 'rgba(255,255,255,0.2)', paddingHorizontal: 32, paddingVertical: 12, borderRadius: 24,
  },
  cancelText: { color: '#fff', fontSize: 16, fontWeight: '600' },
});
```

**Step 4: Build and verify mobile app compiles**

Run: `cd mobile && npx expo start` — verify no compilation errors, connect screen shows "Scan QR Code" button.

**Step 5: Commit**

```bash
git add mobile/package.json mobile/package-lock.json mobile/app.json mobile/app/connect.tsx
git commit -m "feat(mobile): add QR code scanner for server connection"
```

---

### Task 3: Configure pkg for Cross-Platform Binary Build

**Files:**
- Modify: `package.json` (add pkg config, devDependency, build:binary script)

**Step 1: Install @yao-pkg/pkg**

Run: `npm install -D @yao-pkg/pkg`

**Step 2: Add pkg configuration and build script to package.json**

Add to `package.json`:

```json
"scripts": {
  "build": "tsc",
  "build:binary": "tsc && pkg . --compress GZip",
  "dev": "tsx src/cli.ts",
  "test": "vitest run",
  "test:watch": "vitest"
},
"pkg": {
  "targets": ["node18-linux-x64", "node18-linux-arm64", "node18-macos-x64", "node18-macos-arm64"],
  "assets": ["web/**/*"],
  "outputPath": "release"
}
```

**Step 3: Build binary for current platform and test**

Run: `npm run build:binary`
Expected: Creates binaries in `release/` directory.

Test the binary:
Run: `./release/remote-terminal-bridge-macos-arm64 start`
Expected: Server starts, QR code shows, web panel works.

**Step 4: Add `release/` to .gitignore**

Append to `.gitignore`:

```
# Binary releases
release/
```

**Step 5: Update Makefile with binary build target**

Add to `Makefile`:

```makefile
build-binary: ## Build standalone binary for current platform
	npm run build:binary
```

**Step 6: Commit**

```bash
git add package.json package-lock.json .gitignore Makefile
git commit -m "feat: add pkg binary build configuration"
```

---

### Task 4: Create CI Workflow

**Files:**
- Create: `.github/workflows/ci.yml`

**Step 1: Create CI workflow file**

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: 18
          cache: npm

      - run: npm ci

      - name: Build
        run: npm run build

      - name: Test
        run: npm test
```

**Step 2: Commit**

```bash
mkdir -p .github/workflows
git add .github/workflows/ci.yml
git commit -m "ci: add CI workflow for build and test"
```

---

### Task 5: Create Release Workflow

**Files:**
- Create: `.github/workflows/release.yml`

**Step 1: Create release workflow**

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags: ['v*']
  workflow_dispatch:
    inputs:
      version:
        description: 'Version to release (e.g. 0.1.0)'
        required: true

permissions:
  contents: write

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: node18-linux-x64
            artifact: rtb-linux-x64
          - os: ubuntu-24.04-arm
            target: node18-linux-arm64
            artifact: rtb-linux-arm64
          - os: macos-13
            target: node18-macos-x64
            artifact: rtb-darwin-x64
          - os: macos-14
            target: node18-macos-arm64
            artifact: rtb-darwin-arm64

    runs-on: ${{ matrix.os }}

    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: 18
          cache: npm

      - run: npm ci

      - name: Build TypeScript
        run: npm run build

      - name: Build binary
        run: npx pkg . --target ${{ matrix.target }} --output release/rtb --compress GZip

      - name: Package
        run: |
          cd release
          tar czf ${{ matrix.artifact }}.tar.gz rtb
          ls -lh ${{ matrix.artifact }}.tar.gz

      - uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.artifact }}
          path: release/${{ matrix.artifact }}.tar.gz

  release:
    needs: build
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      - uses: actions/download-artifact@v4
        with:
          path: artifacts
          merge-multiple: true

      - name: Determine version
        id: version
        run: |
          if [ "${{ github.event_name }}" = "push" ]; then
            echo "tag=${GITHUB_REF#refs/tags/}" >> "$GITHUB_OUTPUT"
          else
            echo "tag=v${{ github.event.inputs.version }}" >> "$GITHUB_OUTPUT"
          fi

      - name: Create Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: ${{ steps.version.outputs.tag }}
          name: RTB ${{ steps.version.outputs.tag }}
          generate_release_notes: true
          files: artifacts/*.tar.gz
```

**Step 2: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add release workflow for cross-platform binary builds"
```

---

### Task 6: Update README and Makefile

**Files:**
- Modify: `README.md` (add installation from release section)
- Modify: `Makefile` (add build-binary target — already done in Task 3)

**Step 1: Add installation section to README.md**

Add after the "## 快速开始" section:

```markdown
## 安装

### 下载预编译二进制

从 [GitHub Releases](https://github.com/codeweiz/remote-desktop-control/releases) 下载对应平台的二进制文件：

\```bash
# macOS (Apple Silicon)
curl -fsSL https://github.com/codeweiz/remote-desktop-control/releases/latest/download/rtb-darwin-arm64.tar.gz | tar xz
./rtb start

# macOS (Intel)
curl -fsSL https://github.com/codeweiz/remote-desktop-control/releases/latest/download/rtb-darwin-x64.tar.gz | tar xz
./rtb start

# Linux (x64)
curl -fsSL https://github.com/codeweiz/remote-desktop-control/releases/latest/download/rtb-linux-x64.tar.gz | tar xz
./rtb start

# Linux (ARM64)
curl -fsSL https://github.com/codeweiz/remote-desktop-control/releases/latest/download/rtb-linux-arm64.tar.gz | tar xz
./rtb start
\```

### 从源码构建

\```bash
make install
make start
\```
```

Also add to mobile section:

```markdown
Mobile App 支持扫码连接：服务器启动后终端会显示 QR Code，在 App 连接页点击 "Scan QR Code" 扫码即可自动连接。
```

**Step 2: Commit**

```bash
git add README.md
git commit -m "docs: add binary installation instructions and QR code usage"
```

---

### Task 7: End-to-End Verification

**Step 1: Run full build**

Run: `npm run build`
Expected: No TypeScript errors.

**Step 2: Run tests**

Run: `npm test`
Expected: All tests pass.

**Step 3: Test binary build**

Run: `npm run build:binary`
Expected: Binary created in `release/`.

**Step 4: Test binary runs**

Run: `./release/remote-terminal-bridge-macos-arm64 start`
Expected: Server starts, QR code displayed, web panel accessible.

**Step 5: Final commit (if any remaining changes)**

```bash
git add -A
git commit -m "chore: final adjustments for CI/CD and packaging"
```
