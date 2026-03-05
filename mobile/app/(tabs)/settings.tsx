import { useState, useEffect } from 'react';
import { View, Text, ScrollView, TouchableOpacity, StyleSheet, Alert } from 'react-native';
import { useRouter } from 'expo-router';
import { useServer } from '../../hooks/useServer';

export default function SettingsScreen() {
  const router = useRouter();
  const { config, baseUrl, clear } = useServer();
  const [serverInfo, setServerInfo] = useState<{ port: number; tunnel: any; feishu: any } | null>(null);

  useEffect(() => {
    if (!baseUrl) return;
    fetch(`${baseUrl}/api/settings`)
      .then((r) => r.json())
      .then(setServerInfo)
      .catch(() => {});
  }, [baseUrl]);

  function handleDisconnect() {
    Alert.alert('Disconnect', 'Are you sure?', [
      { text: 'Cancel', style: 'cancel' },
      { text: 'Disconnect', style: 'destructive', onPress: async () => {
        await clear();
        router.replace('/connect');
      }},
    ]);
  }

  return (
    <ScrollView style={styles.container} contentContainerStyle={styles.content}>
      <Text style={styles.sectionTitle}>Connection</Text>
      <View style={styles.card}>
        <View style={styles.row}>
          <Text style={styles.label}>Server</Text>
          <Text style={styles.value}>{config?.address || '—'}</Text>
        </View>
        <View style={styles.row}>
          <Text style={styles.label}>HTTPS</Text>
          <Text style={styles.value}>{config?.useSSL ? 'Yes' : 'No'}</Text>
        </View>
      </View>

      {serverInfo && (
        <>
          <Text style={styles.sectionTitle}>Server Info</Text>
          <View style={styles.card}>
            <View style={styles.row}>
              <Text style={styles.label}>Port</Text>
              <Text style={styles.value}>{serverInfo.port}</Text>
            </View>
            <View style={styles.row}>
              <Text style={styles.label}>Tunnel</Text>
              <Text style={styles.value}>{serverInfo.tunnel?.hostname || 'not configured'}</Text>
            </View>
            <View style={styles.row}>
              <Text style={styles.label}>Feishu</Text>
              <Text style={styles.value}>{serverInfo.feishu?.configured ? 'connected' : 'not configured'}</Text>
            </View>
          </View>
        </>
      )}

      <TouchableOpacity style={styles.disconnectBtn} onPress={handleDisconnect}>
        <Text style={styles.disconnectText}>Disconnect from Server</Text>
      </TouchableOpacity>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: '#0d1117' },
  content: { padding: 16 },
  sectionTitle: {
    fontSize: 12, color: '#8b949e', fontWeight: '600',
    textTransform: 'uppercase', letterSpacing: 0.5,
    marginTop: 20, marginBottom: 8,
  },
  card: {
    backgroundColor: '#161b22', borderRadius: 10,
    borderWidth: 1, borderColor: '#30363d', overflow: 'hidden',
  },
  row: {
    flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center',
    padding: 14, borderBottomWidth: 1, borderBottomColor: '#21262d',
  },
  label: { fontSize: 15, color: '#c9d1d9' },
  value: { fontSize: 14, color: '#8b949e', fontFamily: 'Menlo' },
  disconnectBtn: {
    marginTop: 32, padding: 14, borderRadius: 8,
    backgroundColor: '#da36331a', alignItems: 'center',
  },
  disconnectText: { color: '#f85149', fontSize: 15, fontWeight: '600' },
});
