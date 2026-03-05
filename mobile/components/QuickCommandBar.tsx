import { ScrollView, TouchableOpacity, Text, StyleSheet } from 'react-native';

const COMMANDS = [
  { label: 'yes', command: 'yes\n', color: '#2ea043' },
  { label: 'no', command: 'no\n', color: '#da3633' },
  { label: 'Ctrl+C', command: '\x03', color: '#d29922' },
  { label: 'Ctrl+D', command: '\x04', color: '#d29922' },
  { label: 'Enter', command: '\r', color: '#58a6ff' },
  { label: '/compact', command: '/compact\n', color: '#8b949e' },
  { label: '/clear', command: '/clear\n', color: '#8b949e' },
  { label: '/help', command: '/help\n', color: '#8b949e' },
];

interface Props {
  onCommand: (command: string) => void;
}

export function QuickCommandBar({ onCommand }: Props) {
  return (
    <ScrollView horizontal style={styles.bar} contentContainerStyle={styles.content} showsHorizontalScrollIndicator={false}>
      {COMMANDS.map((cmd) => (
        <TouchableOpacity key={cmd.label} style={[styles.btn, { borderColor: cmd.color }]} onPress={() => onCommand(cmd.command)}>
          <Text style={[styles.btnText, { color: cmd.color }]}>{cmd.label}</Text>
        </TouchableOpacity>
      ))}
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  bar: { backgroundColor: '#161b22', borderTopWidth: 1, borderTopColor: '#30363d', maxHeight: 48 },
  content: { alignItems: 'center', paddingHorizontal: 8, gap: 6 },
  btn: {
    paddingHorizontal: 14, paddingVertical: 6, borderRadius: 6,
    backgroundColor: '#21262d', borderWidth: 1,
  },
  btnText: { fontSize: 12, fontFamily: 'Menlo' },
});
