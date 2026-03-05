import { useState, useEffect, useCallback, useMemo } from 'react';
import { View, TextInput, TouchableOpacity, Text, StyleSheet } from 'react-native';
import { useRouter } from 'expo-router';
import { Ionicons } from '@expo/vector-icons';
import { useServer } from '../../hooks/useServer';
import { SessionList, SessionInfo } from '../../components/SessionList';
import { CreateSessionModal } from '../../components/CreateSessionModal';

export default function SessionsScreen() {
  const router = useRouter();
  const { baseUrl } = useServer();
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [modalVisible, setModalVisible] = useState(false);
  const [search, setSearch] = useState('');

  const fetchSessions = useCallback(async () => {
    if (!baseUrl) return;
    try {
      const res = await fetch(`${baseUrl}/api/sessions`);
      setSessions(await res.json());
    } catch (e) {
      console.error('Failed to fetch sessions:', e);
    }
  }, [baseUrl]);

  useEffect(() => {
    fetchSessions();
    const interval = setInterval(fetchSessions, 3000);
    return () => clearInterval(interval);
  }, [fetchSessions]);

  const filtered = useMemo(() => {
    if (!search.trim()) return sessions;
    const q = search.toLowerCase();
    return sessions.filter(s =>
      s.name.toLowerCase().includes(q) ||
      s.command.toLowerCase().includes(q) ||
      (s.lastLine && s.lastLine.toLowerCase().includes(q))
    );
  }, [sessions, search]);

  async function handleCreate(name: string, command: string, argsStr: string) {
    if (!baseUrl) return;
    const args = argsStr ? argsStr.split(/\s+/).filter(Boolean) : [];
    try {
      await fetch(`${baseUrl}/api/sessions`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name, command, args }),
      });
      await fetchSessions();
    } catch {
      // Alert handled by SessionList
    }
  }

  async function handleDelete(id: string) {
    if (!baseUrl) return;
    try {
      await fetch(`${baseUrl}/api/sessions/${id}`, { method: 'DELETE' });
      await fetchSessions();
    } catch {
      // silent
    }
  }

  function handleSelect(id: string) {
    router.push({ pathname: '/(tabs)/terminal', params: { sessionId: id } });
  }

  return (
    <View style={styles.container}>
      <View style={styles.searchBar}>
        <Ionicons name="search" size={16} color="#484f58" style={styles.searchIcon} />
        <TextInput
          style={styles.searchInput}
          placeholder="Search sessions..."
          placeholderTextColor="#484f58"
          value={search}
          onChangeText={setSearch}
          autoCapitalize="none"
          autoCorrect={false}
        />
        {search.length > 0 && (
          <TouchableOpacity onPress={() => setSearch('')} style={styles.clearBtn}>
            <Ionicons name="close-circle" size={16} color="#484f58" />
          </TouchableOpacity>
        )}
      </View>
      <SessionList sessions={filtered} onSelect={handleSelect} onDelete={handleDelete} />
      <TouchableOpacity style={styles.fab} onPress={() => setModalVisible(true)}>
        <Text style={styles.fabText}>+</Text>
      </TouchableOpacity>
      <CreateSessionModal
        visible={modalVisible}
        onClose={() => setModalVisible(false)}
        onCreate={handleCreate}
      />
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: '#0d1117' },
  searchBar: {
    flexDirection: 'row', alignItems: 'center',
    marginHorizontal: 12, marginTop: 8, marginBottom: 4,
    backgroundColor: '#161b22', borderWidth: 1, borderColor: '#30363d',
    borderRadius: 8, paddingHorizontal: 10,
  },
  searchIcon: { marginRight: 6 },
  searchInput: { flex: 1, color: '#c9d1d9', fontSize: 14, paddingVertical: 8 },
  clearBtn: { padding: 4 },
  fab: {
    position: 'absolute', bottom: 20, right: 20,
    width: 56, height: 56, borderRadius: 28,
    backgroundColor: '#238636', alignItems: 'center', justifyContent: 'center',
    shadowColor: '#000', shadowOffset: { width: 0, height: 2 },
    shadowOpacity: 0.3, shadowRadius: 4, elevation: 4,
  },
  fabText: { color: '#fff', fontSize: 28, fontWeight: '300', marginTop: -2 },
});
