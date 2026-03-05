import { useState, useEffect, useCallback } from 'react';
import { View, TouchableOpacity, Text, StyleSheet, Alert } from 'react-native';
import { useRouter } from 'expo-router';
import { useServer } from '../../hooks/useServer';
import { SessionList, SessionInfo } from '../../components/SessionList';
import { CreateSessionModal } from '../../components/CreateSessionModal';

export default function SessionsScreen() {
  const router = useRouter();
  const { baseUrl } = useServer();
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [modalVisible, setModalVisible] = useState(false);

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
    } catch (e) {
      Alert.alert('Error', 'Failed to create session');
    }
  }

  async function handleDelete(id: string) {
    if (!baseUrl) return;
    try {
      await fetch(`${baseUrl}/api/sessions/${id}`, { method: 'DELETE' });
      await fetchSessions();
    } catch (e) {
      Alert.alert('Error', 'Failed to delete session');
    }
  }

  function handleSelect(id: string) {
    router.push({ pathname: '/(tabs)/terminal', params: { sessionId: id } });
  }

  return (
    <View style={styles.container}>
      <SessionList sessions={sessions} onSelect={handleSelect} onDelete={handleDelete} />
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
  fab: {
    position: 'absolute', bottom: 20, right: 20,
    width: 56, height: 56, borderRadius: 28,
    backgroundColor: '#238636', alignItems: 'center', justifyContent: 'center',
    shadowColor: '#000', shadowOffset: { width: 0, height: 2 },
    shadowOpacity: 0.3, shadowRadius: 4, elevation: 4,
  },
  fabText: { color: '#fff', fontSize: 28, fontWeight: '300', marginTop: -2 },
});
