import { useState, useEffect } from 'react';
import { View, Text, ScrollView, TouchableOpacity, StyleSheet, Alert, Platform } from 'react-native';
import { useRouter } from 'expo-router';
import Slider from '@react-native-community/slider';
import { useServer } from '../../hooks/useServer';
import { useTerminalSettings, THEME_LABELS, THEME_COLORS } from '../../hooks/useTerminalSettings';
import { useTheme } from '../../contexts/ThemeContext';
import type { AppThemeMode } from '../../hooks/useAppTheme';

const APP_THEME_OPTIONS: { key: AppThemeMode; label: string; icon: string }[] = [
  { key: 'light', label: 'Light', icon: '☀️' },
  { key: 'dark', label: 'Dark', icon: '🌙' },
  { key: 'system', label: 'System', icon: '⚙️' },
];

export default function SettingsScreen() {
  const router = useRouter();
  const { config, baseUrl, clear } = useServer();
  const { settings, update } = useTerminalSettings();
  const { mode: appThemeMode, updateMode: setAppThemeMode, colors } = useTheme();
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
    <ScrollView style={[styles.container, { backgroundColor: colors.background }]} contentContainerStyle={styles.content}>
      <Text style={[styles.sectionTitle, { color: colors.textSecondary }]}>Appearance</Text>
      <View style={[styles.card, { backgroundColor: colors.cardBg, borderColor: colors.cardBorder }]}>
        <View style={styles.themeRow}>
          {APP_THEME_OPTIONS.map((opt) => {
            const isActive = appThemeMode === opt.key;
            return (
              <TouchableOpacity
                key={opt.key}
                style={[
                  styles.appThemeOption,
                  { borderColor: isActive ? colors.accent : 'transparent',
                    backgroundColor: isActive ? colors.accent + '22' : 'transparent' },
                ]}
                onPress={() => setAppThemeMode(opt.key)}
              >
                <Text style={styles.appThemeIcon}>{opt.icon}</Text>
                <Text style={[styles.appThemeLabel, { color: isActive ? colors.accent : colors.textSecondary }]}>
                  {opt.label}
                </Text>
              </TouchableOpacity>
            );
          })}
        </View>
      </View>

      <Text style={[styles.sectionTitle, { color: colors.textSecondary }]}>Terminal Display</Text>
      <View style={[styles.card, { backgroundColor: colors.cardBg, borderColor: colors.cardBorder }]}>
        <View style={[styles.row, { borderBottomColor: colors.surfaceBorder }]}>
          <Text style={[styles.label, { color: colors.text }]}>Font Size</Text>
          <Text style={[styles.value, { color: colors.textSecondary }]}>{settings.fontSize}px</Text>
        </View>
        <View style={[styles.sliderRow, { borderBottomColor: colors.surfaceBorder }]}>
          <Text style={[styles.sliderLabel, { color: colors.textMuted }]}>12</Text>
          <Slider
            style={styles.slider}
            minimumValue={12}
            maximumValue={24}
            step={1}
            value={settings.fontSize}
            onValueChange={(v) => update({ fontSize: v })}
            minimumTrackTintColor={colors.accent}
            maximumTrackTintColor={colors.surfaceBorder}
            thumbTintColor={colors.accent}
          />
          <Text style={[styles.sliderLabel, { color: colors.textMuted }]}>24</Text>
        </View>
        <View style={[styles.row, { borderBottomWidth: 0 }]}>
          <Text style={[styles.label, { color: colors.text }]}>Color Scheme</Text>
        </View>
        <View style={styles.themeGrid}>
          {themeKeys.map((key) => {
            const tc = THEME_COLORS[key];
            const isActive = settings.theme === key;
            return (
              <TouchableOpacity
                key={key}
                style={[
                  styles.themeOption,
                  { borderColor: isActive ? colors.accent : 'transparent',
                    backgroundColor: isActive ? colors.accent + '22' : 'transparent' },
                ]}
                onPress={() => update({ theme: key as any })}
              >
                <View style={[styles.themePreview, { backgroundColor: tc.background, borderColor: colors.surfaceBorder }]}>
                  <Text style={{ color: tc.foreground, fontSize: 10, fontFamily: Platform.select({ ios: 'Menlo', android: 'monospace' }) }}>
                    Aa
                  </Text>
                </View>
                <Text style={[styles.themeName, { color: isActive ? colors.accent : colors.textSecondary }]}>
                  {THEME_LABELS[key]}
                </Text>
              </TouchableOpacity>
            );
          })}
        </View>
      </View>

      <Text style={[styles.sectionTitle, { color: colors.textSecondary }]}>Connection</Text>
      <View style={[styles.card, { backgroundColor: colors.cardBg, borderColor: colors.cardBorder }]}>
        <View style={[styles.row, { borderBottomColor: colors.surfaceBorder }]}>
          <Text style={[styles.label, { color: colors.text }]}>Server</Text>
          <Text style={[styles.value, { color: colors.textSecondary }]}>{config?.address || '—'}</Text>
        </View>
        <View style={[styles.row, { borderBottomColor: colors.surfaceBorder }]}>
          <Text style={[styles.label, { color: colors.text }]}>HTTPS</Text>
          <Text style={[styles.value, { color: colors.textSecondary }]}>{config?.useSSL ? 'Yes' : 'No'}</Text>
        </View>
      </View>

      {serverInfo && (
        <>
          <Text style={[styles.sectionTitle, { color: colors.textSecondary }]}>Server Info</Text>
          <View style={[styles.card, { backgroundColor: colors.cardBg, borderColor: colors.cardBorder }]}>
            <View style={[styles.row, { borderBottomColor: colors.surfaceBorder }]}>
              <Text style={[styles.label, { color: colors.text }]}>Port</Text>
              <Text style={[styles.value, { color: colors.textSecondary }]}>{serverInfo.port}</Text>
            </View>
            <View style={[styles.row, { borderBottomColor: colors.surfaceBorder }]}>
              <Text style={[styles.label, { color: colors.text }]}>Tunnel</Text>
              <Text style={[styles.value, { color: colors.textSecondary }]}>{serverInfo.tunnel?.hostname || 'not configured'}</Text>
            </View>
            <View style={[styles.row, { borderBottomColor: colors.surfaceBorder }]}>
              <Text style={[styles.label, { color: colors.text }]}>Feishu</Text>
              <Text style={[styles.value, { color: colors.textSecondary }]}>{serverInfo.feishu?.configured ? 'connected' : 'not configured'}</Text>
            </View>
          </View>
        </>
      )}

      <TouchableOpacity style={[styles.disconnectBtn, { backgroundColor: colors.dangerBg }]} onPress={handleDisconnect}>
        <Text style={[styles.disconnectText, { color: colors.danger }]}>Disconnect from Server</Text>
      </TouchableOpacity>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1 },
  content: { padding: 16, paddingBottom: 40 },
  sectionTitle: {
    fontSize: 12, fontWeight: '600',
    textTransform: 'uppercase', letterSpacing: 0.5,
    marginTop: 20, marginBottom: 8,
  },
  card: {
    borderRadius: 10, borderWidth: 1, overflow: 'hidden',
  },
  row: {
    flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center',
    padding: 14, borderBottomWidth: 1,
  },
  label: { fontSize: 15 },
  value: { fontSize: 14, fontFamily: Platform.select({ ios: 'Menlo', android: 'monospace' }) },
  sliderRow: {
    flexDirection: 'row', alignItems: 'center',
    paddingHorizontal: 14, paddingBottom: 12,
    borderBottomWidth: 1,
  },
  slider: { flex: 1, marginHorizontal: 8 },
  sliderLabel: { fontSize: 11 },
  themeGrid: {
    flexDirection: 'row', flexWrap: 'wrap', gap: 10,
    padding: 14,
  },
  themeOption: {
    alignItems: 'center', gap: 6,
    padding: 8, borderRadius: 8,
    borderWidth: 1, width: '22%',
  },
  themePreview: {
    width: 40, height: 28, borderRadius: 6,
    justifyContent: 'center', alignItems: 'center',
    borderWidth: 1,
  },
  themeName: { fontSize: 11 },
  themeRow: {
    flexDirection: 'row', justifyContent: 'space-around',
    padding: 14,
  },
  appThemeOption: {
    alignItems: 'center', gap: 4,
    paddingVertical: 10, paddingHorizontal: 20,
    borderRadius: 10, borderWidth: 1.5,
  },
  appThemeIcon: { fontSize: 22 },
  appThemeLabel: { fontSize: 12, fontWeight: '500' },
  disconnectBtn: {
    marginTop: 32, padding: 14, borderRadius: 8,
    alignItems: 'center',
  },
  disconnectText: { fontSize: 15, fontWeight: '600' },
});
