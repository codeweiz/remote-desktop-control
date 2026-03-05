import { View, Text, FlatList, TouchableOpacity, StyleSheet } from 'react-native';

export interface SessionInfo {
  id: string;
  name: string;
  command: string;
  args: string[];
  status: 'running' | 'exited' | 'waiting-input';
  exitCode?: number;
  createdAt: string;
}

interface Props {
  sessions: SessionInfo[];
  onSelect: (id: string) => void;
  onDelete: (id: string) => void;
}

const STATUS_COLORS = {
  running: '#2ea043',
  'waiting-input': '#d29922',
  exited: '#484f58',
};

export function SessionList({ sessions, onSelect, onDelete }: Props) {
  return (
    <FlatList
      data={sessions}
      keyExtractor={(item) => item.id}
      contentContainerStyle={styles.list}
      renderItem={({ item }) => (
        <TouchableOpacity style={styles.item} onPress={() => onSelect(item.id)}>
          <View style={[styles.dot, { backgroundColor: STATUS_COLORS[item.status] }]} />
          <View style={styles.info}>
            <Text style={styles.name}>{item.name}</Text>
            <Text style={styles.status}>{item.status}</Text>
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
  status: { color: '#8b949e', fontSize: 12, marginTop: 2 },
  deleteBtn: { padding: 4 },
  deleteText: { color: '#8b949e', fontSize: 20 },
  empty: { alignItems: 'center', paddingTop: 60 },
  emptyText: { color: '#484f58', fontSize: 15 },
});
