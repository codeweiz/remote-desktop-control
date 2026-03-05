import { View, Text, FlatList, TouchableOpacity, StyleSheet, Platform } from 'react-native';
import type { AppColors } from '../hooks/useAppTheme';

export interface SessionInfo {
  id: string;
  name: string;
  command: string;
  args: string[];
  status: 'running' | 'exited' | 'waiting-input';
  exitCode?: number;
  createdAt: string;
  lastLine?: string;
}

interface Props {
  sessions: SessionInfo[];
  onSelect: (id: string) => void;
  onDelete: (id: string) => void;
  colors: AppColors;
}

const STATUS_COLORS: Record<string, string> = {
  running: '#2ea043',
  'waiting-input': '#d29922',
  exited: '#484f58',
};

function cleanLastLine(line: string): string {
  return line.replace(/\x1b\[[0-9;]*[a-zA-Z]/g, '').trim().slice(-80);
}

export function SessionList({ sessions, onSelect, onDelete, colors }: Props) {
  return (
    <FlatList
      data={sessions}
      keyExtractor={(item) => item.id}
      contentContainerStyle={styles.list}
      renderItem={({ item }) => (
        <TouchableOpacity
          style={[styles.item, { backgroundColor: colors.cardBg, borderColor: colors.cardBorder }]}
          onPress={() => onSelect(item.id)}
        >
          <View style={[styles.dot, { backgroundColor: STATUS_COLORS[item.status] || '#484f58' }]} />
          <View style={styles.info}>
            <Text style={[styles.name, { color: colors.text }]}>{item.name}</Text>
            <Text style={[styles.lastLine, { color: colors.textSecondary }]} numberOfLines={1}>
              {item.lastLine ? cleanLastLine(item.lastLine) : `${item.command} ${item.args.join(' ')}`.trim() || 'shell'}
            </Text>
          </View>
          <TouchableOpacity style={styles.deleteBtn} onPress={() => onDelete(item.id)}>
            <Text style={[styles.deleteText, { color: colors.textSecondary }]}>×</Text>
          </TouchableOpacity>
        </TouchableOpacity>
      )}
      ListEmptyComponent={
        <View style={styles.empty}>
          <Text style={[styles.emptyText, { color: colors.textMuted }]}>No sessions yet</Text>
        </View>
      }
    />
  );
}

const styles = StyleSheet.create({
  list: { padding: 12 },
  item: {
    flexDirection: 'row', alignItems: 'center', gap: 12,
    padding: 14, marginBottom: 8, borderRadius: 10,
    borderWidth: 1,
  },
  dot: { width: 10, height: 10, borderRadius: 5 },
  info: { flex: 1 },
  name: { fontSize: 15, fontWeight: '500' },
  lastLine: {
    fontSize: 11, marginTop: 3,
    fontFamily: Platform.select({ ios: 'Menlo', android: 'monospace' }),
  },
  deleteBtn: { padding: 4 },
  deleteText: { fontSize: 20 },
  empty: { alignItems: 'center', paddingTop: 60 },
  emptyText: { fontSize: 15 },
});
