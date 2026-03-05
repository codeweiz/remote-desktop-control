import { View, Text, FlatList, TouchableOpacity, StyleSheet, Platform } from 'react-native';

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
}

const STATUS_COLORS: Record<string, string> = {
  running: '#2ea043',
  'waiting-input': '#d29922',
  exited: '#484f58',
};

function cleanLastLine(line: string): string {
  // Strip ANSI escape codes for display
  return line.replace(/\x1b\[[0-9;]*[a-zA-Z]/g, '').trim().slice(-80);
}

export function SessionList({ sessions, onSelect, onDelete }: Props) {
  return (
    <FlatList
      data={sessions}
      keyExtractor={(item) => item.id}
      contentContainerStyle={styles.list}
      renderItem={({ item }) => (
        <TouchableOpacity style={styles.item} onPress={() => onSelect(item.id)}>
          <View style={[styles.dot, { backgroundColor: STATUS_COLORS[item.status] || '#484f58' }]} />
          <View style={styles.info}>
            <Text style={styles.name}>{item.name}</Text>
            <Text style={styles.lastLine} numberOfLines={1}>
              {item.lastLine ? cleanLastLine(item.lastLine) : `${item.command} ${item.args.join(' ')}`.trim()}
            </Text>
          </View>
          <TouchableOpacity style={styles.deleteBtn} onPress={() => onDelete(item.id)}>
            <Text style={styles.deleteText}>×</Text>
          </TouchableOpacity>
        </TouchableOpacity>
      )}
      ListEmptyComponent={
        <View style={styles.empty}>
          <Text style={styles.emptyText}>No sessions yet</Text>
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
    backgroundColor: '#161b22', borderWidth: 1, borderColor: '#30363d',
  },
  dot: { width: 10, height: 10, borderRadius: 5 },
  info: { flex: 1 },
  name: { color: '#c9d1d9', fontSize: 15, fontWeight: '500' },
  lastLine: {
    color: '#8b949e', fontSize: 11, marginTop: 3,
    fontFamily: Platform.select({ ios: 'Menlo', android: 'monospace' }),
  },
  deleteBtn: { padding: 4 },
  deleteText: { color: '#8b949e', fontSize: 20 },
  empty: { alignItems: 'center', paddingTop: 60 },
  emptyText: { color: '#484f58', fontSize: 15 },
});
