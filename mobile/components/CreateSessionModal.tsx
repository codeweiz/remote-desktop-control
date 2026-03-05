import { useState } from 'react';
import { View, Text, TextInput, TouchableOpacity, Modal, StyleSheet } from 'react-native';

interface Props {
  visible: boolean;
  onClose: () => void;
  onCreate: (name: string, command: string, args: string) => void;
}

export function CreateSessionModal({ visible, onClose, onCreate }: Props) {
  const [name, setName] = useState('');
  const [command, setCommand] = useState('');
  const [args, setArgs] = useState('');

  function handleCreate() {
    if (!name.trim() || !command.trim()) return;
    onCreate(name.trim(), command.trim(), args.trim());
    setName('');
    setCommand('');
    setArgs('');
    onClose();
  }

  return (
    <Modal visible={visible} transparent animationType="fade">
      <View style={styles.overlay}>
        <View style={styles.content}>
          <Text style={styles.title}>Create Session</Text>
          <Text style={styles.label}>Name</Text>
          <TextInput style={styles.input} placeholder="my-session" placeholderTextColor="#484f58"
            value={name} onChangeText={setName} autoCapitalize="none" />
          <Text style={styles.label}>Command</Text>
          <TextInput style={styles.input} placeholder="claude" placeholderTextColor="#484f58"
            value={command} onChangeText={setCommand} autoCapitalize="none" />
          <Text style={styles.label}>Arguments (optional)</Text>
          <TextInput style={styles.input} placeholder="--verbose" placeholderTextColor="#484f58"
            value={args} onChangeText={setArgs} autoCapitalize="none" />
          <View style={styles.actions}>
            <TouchableOpacity style={styles.cancelBtn} onPress={onClose}>
              <Text style={styles.cancelText}>Cancel</Text>
            </TouchableOpacity>
            <TouchableOpacity style={styles.createBtn} onPress={handleCreate}>
              <Text style={styles.createText}>Create</Text>
            </TouchableOpacity>
          </View>
        </View>
      </View>
    </Modal>
  );
}

const styles = StyleSheet.create({
  overlay: { flex: 1, backgroundColor: 'rgba(0,0,0,.6)', justifyContent: 'center', padding: 24 },
  content: { backgroundColor: '#161b22', borderRadius: 12, padding: 24, borderWidth: 1, borderColor: '#30363d' },
  title: { fontSize: 18, fontWeight: '700', color: '#f0f6fc', marginBottom: 20 },
  label: { fontSize: 12, color: '#8b949e', fontWeight: '500', marginBottom: 4, marginTop: 8 },
  input: {
    backgroundColor: '#0d1117', borderWidth: 1, borderColor: '#30363d', borderRadius: 8,
    padding: 10, color: '#c9d1d9', fontSize: 14,
  },
  actions: { flexDirection: 'row', justifyContent: 'flex-end', gap: 8, marginTop: 20 },
  cancelBtn: { paddingHorizontal: 16, paddingVertical: 8, borderRadius: 6, backgroundColor: '#21262d' },
  cancelText: { color: '#c9d1d9', fontSize: 14 },
  createBtn: { paddingHorizontal: 16, paddingVertical: 8, borderRadius: 6, backgroundColor: '#238636' },
  createText: { color: '#fff', fontSize: 14, fontWeight: '600' },
});
