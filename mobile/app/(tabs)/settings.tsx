import { useState, useEffect } from 'react';
import { View, Text, ScrollView, TouchableOpacity, StyleSheet, Alert, Platform } from 'react-native';
import { useRouter } from 'expo-router';
import Slider from '@react-native-community/slider';
import { useServer } from '../../hooks/useServer';
import { useTerminalSettings, THEME_LABELS, THEME_COLORS } from '../../hooks/useTerminalSettings';

export default function SettingsScreen() {
  const router = useRouter();
  const { config, baseUrl, clear } = useServer();
  const { settings, update } = useTerminalSettings();
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

  const themeKeys = Object.keys(THEME_LABELS);

  return (
    <ScrollView style={styles.container} contentContainerStyle={styles.content}>
      <Text style={styles.sectionTitle}>Terminal Display</Text>
      <View style={styles.card}>
        <View style={styles.row}>
          <Text style={styles.label}>Font Size</Text>
          <Text style={styles.value}>{settings.fontSize}px</Text>
        </View>
        <View style={styles.sliderRow}>
          <Text style={styles.sliderLabel}>12</Text>
          <Slider
            style={styles.slider}
            minimumValue={12}
            maximumValue={24}
            step={1}
            value={settings.fontSize}
            onValueChange={(v) => update({ fontSize: v })}
            minimumTrackTintColor="#58a6ff"
            maximumTrackTintColor="#30363d"
            thumbTintColor="#58a6ff"
          />
          <Text style={styles.sliderLabel}>24</Text>
        </View>
        <View style={[styles.row, { borderBottomWidth: 0 }]}>
          <Text style={styles.label}>Color Scheme</Text>
        </View>
        <View style={styles.themeGrid}>
          {themeKeys.map((key) => {
            const colors = THEME_COLORS[key];
            const isActive = settings.theme === key;
            return (
              <TouchableOpacity
                key={key}
                style={[styles.themeOption, isActive && styles.themeOptionActive]}
                onPress={() => update({ theme: key as any })}
              >
                <View style={[styles.themePreview, { backgroundColor: colors.background }]}>
                  <Text style={{ color: colors.foreground, fontSize: 10, fontFamily: Platform.select({ ios: 'Menlo', android: 'monospace' }) }}>
                    Aa
                  </Text>
                </View>
                <Text style={[styles.themeName, isActive && styles.themeNameActive]}>
                  {THEME_LABELS[key]}
                </Text>
              </TouchableOpacity>
            );
          })}
        </View>
      </View>

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
  content: { padding: 16, paddingBottom: 40 },
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
  value: { fontSize: 14, color: '#8b949e', fontFamily: Platform.select({ ios: 'Menlo', android: 'monospace' }) },
  sliderRow: {
    flexDirection: 'row', alignItems: 'center',
    paddingHorizontal: 14, paddingBottom: 12,
    borderBottomWidth: 1, borderBottomColor: '#21262d',
  },
  slider: { flex: 1, marginHorizontal: 8 },
  sliderLabel: { fontSize: 11, color: '#484f58' },
  themeGrid: {
    flexDirection: 'row', flexWrap: 'wrap', gap: 10,
    padding: 14,
  },
  themeOption: {
    alignItems: 'center', gap: 6,
    padding: 8, borderRadius: 8,
    borderWidth: 1, borderColor: 'transparent',
    width: '22%',
  },
  themeOptionActive: {
    borderColor: '#58a6ff', backgroundColor: '#1f6feb22',
  },
  themePreview: {
    width: 40, height: 28, borderRadius: 6,
    justifyContent: 'center', alignItems: 'center',
    borderWidth: 1, borderColor: '#30363d',
  },
  themeName: { fontSize: 11, color: '#8b949e' },
  themeNameActive: { color: '#58a6ff' },
  disconnectBtn: {
    marginTop: 32, padding: 14, borderRadius: 8,
    backgroundColor: '#da36331a', alignItems: 'center',
  },
  disconnectText: { color: '#f85149', fontSize: 15, fontWeight: '600' },
});
