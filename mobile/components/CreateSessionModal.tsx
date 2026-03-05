import { useState } from 'react';
import { View, Text, TextInput, TouchableOpacity, Modal, StyleSheet } from 'react-native';
import type { AppColors } from '../hooks/useAppTheme';

interface Props {
  visible: boolean;
  onClose: () => void;
  onCreate: (name: string, command: string, args: string) => void;
  colors: AppColors;
}

export function CreateSessionModal({ visible, onClose, onCreate, colors }: Props) {
  const [name, setName] = useState('');
  const [command, setCommand] = useState('');
  const [args, setArgs] = useState('');

  function handleCreate() {
    if (!name.trim()) return;
    onCreate(name.trim(), command.trim(), args.trim());
    setName('');
    setCommand('');
    setArgs('');
    onClose();
  }

  return (
    <Modal visible={visible} transparent animationType="fade">
      <View style={[styles.overlay, { backgroundColor: colors.overlayBg }]}>
        <View style={[styles.content, { backgroundColor: colors.cardBg, borderColor: colors.cardBorder }]}>
          <Text style={[styles.title, { color: colors.headerText }]}>Create Session</Text>
          <Text style={[styles.label, { color: colors.textSecondary }]}>Name</Text>
          <TextInput
            style={[styles.input, { backgroundColor: colors.inputBg, borderColor: colors.inputBorder, color: colors.text }]}
            placeholder="my-session" placeholderTextColor={colors.textMuted}
            value={name} onChangeText={setName} autoCapitalize="none"
          />
          <Text style={[styles.label, { color: colors.textSecondary }]}>Command (optional)</Text>
          <TextInput
            style={[styles.input, { backgroundColor: colors.inputBg, borderColor: colors.inputBorder, color: colors.text }]}
            placeholder="claude (empty = plain shell)" placeholderTextColor={colors.textMuted}
            value={command} onChangeText={setCommand} autoCapitalize="none"
          />
          <Text style={[styles.label, { color: colors.textSecondary }]}>Arguments (optional)</Text>
          <TextInput
            style={[styles.input, { backgroundColor: colors.inputBg, borderColor: colors.inputBorder, color: colors.text }]}
            placeholder="--verbose" placeholderTextColor={colors.textMuted}
            value={args} onChangeText={setArgs} autoCapitalize="none"
          />
          <View style={styles.actions}>
            <TouchableOpacity style={[styles.cancelBtn, { backgroundColor: colors.surfaceBorder }]} onPress={onClose}>
              <Text style={[styles.cancelText, { color: colors.text }]}>Cancel</Text>
            </TouchableOpacity>
            <TouchableOpacity style={[styles.createBtn, { backgroundColor: colors.success }]} onPress={handleCreate}>
              <Text style={styles.createText}>Create</Text>
            </TouchableOpacity>
          </View>
        </View>
      </View>
    </Modal>
  );
}

const styles = StyleSheet.create({
  overlay: { flex: 1, justifyContent: 'center', padding: 24 },
  content: { borderRadius: 12, padding: 24, borderWidth: 1 },
  title: { fontSize: 18, fontWeight: '700', marginBottom: 20 },
  label: { fontSize: 12, fontWeight: '500', marginBottom: 4, marginTop: 8 },
  input: {
    borderWidth: 1, borderRadius: 8, padding: 10, fontSize: 14,
  },
  actions: { flexDirection: 'row', justifyContent: 'flex-end', gap: 8, marginTop: 20 },
  cancelBtn: { paddingHorizontal: 16, paddingVertical: 8, borderRadius: 6 },
  cancelText: { fontSize: 14 },
  createBtn: { paddingHorizontal: 16, paddingVertical: 8, borderRadius: 6 },
  createText: { color: '#fff', fontSize: 14, fontWeight: '600' },
});
