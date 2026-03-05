import { useState } from 'react';
import { View, Text, TextInput, TouchableOpacity, StyleSheet, Alert } from 'react-native';
import { useRouter } from 'expo-router';
import { useServer } from '../hooks/useServer';

export default function ConnectScreen() {
  const router = useRouter();
  const { save } = useServer();
  const [address, setAddress] = useState('');
  const [token, setToken] = useState('');
  const [useSSL, setUseSSL] = useState(true);

  async function handleConnect() {
    if (!address.trim() || !token.trim()) {
      Alert.alert('Error', 'Please enter server address and token');
      return;
    }

    const config = { address: address.trim(), token: token.trim(), useSSL };

    // Test connection
    try {
      const base = `${useSSL ? 'https' : 'http'}://${config.address}`;
      const res = await fetch(`${base}/api/sessions`);
      if (!res.ok) throw new Error('Connection failed');
    } catch {
      Alert.alert('Connection Failed', 'Could not reach the server. Check address and try again.');
      return;
    }

    await save(config);
    router.replace('/(tabs)/sessions');
  }

  return (
    <View style={styles.container}>
      <Text style={styles.title}>RTB</Text>
      <Text style={styles.subtitle}>Remote Terminal Bridge</Text>

      <View style={styles.form}>
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

        <TouchableOpacity style={styles.connectBtn} onPress={handleConnect}>
          <Text style={styles.connectText}>Connect</Text>
        </TouchableOpacity>
      </View>
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
  connectBtn: {
    backgroundColor: '#238636', padding: 14, borderRadius: 8,
    alignItems: 'center', marginTop: 16,
  },
  connectText: { color: '#fff', fontSize: 16, fontWeight: '600' },
});
