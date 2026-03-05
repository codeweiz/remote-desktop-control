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
