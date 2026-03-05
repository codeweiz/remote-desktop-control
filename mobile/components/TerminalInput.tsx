import { useRef, useState } from 'react';
import { View, TextInput, ScrollView, TouchableOpacity, Text, StyleSheet, Platform } from 'react-native';

const QUICK_COMMANDS = [
  { label: 'ESC', command: '\x1b', color: '#d29922' },
  { label: 'Tab', command: '\t', color: '#58a6ff' },
  { label: 'Ctrl+C', command: '\x03', color: '#da3633' },
  { label: 'Ctrl+D', command: '\x04', color: '#d29922' },
  { label: 'Ctrl+Z', command: '\x1a', color: '#d29922' },
  { label: 'Ctrl+L', command: '\x0c', color: '#8b949e' },
  { label: '↑', command: '\x1b[A', color: '#8b949e' },
  { label: '↓', command: '\x1b[B', color: '#8b949e' },
  { label: 'yes', command: 'yes\n', color: '#2ea043' },
  { label: 'no', command: 'no\n', color: '#da3633' },
];

interface Props {
  onInput: (data: string) => void;
}

export function TerminalInput({ onInput }: Props) {
  const inputRef = useRef<TextInput>(null);
  const [text, setText] = useState('');

  function handleSubmit() {
    onInput(text + '\n');
    setText('');
  }

  function handleQuickCommand(command: string) {
    onInput(command);
    // Re-focus input after quick command
    inputRef.current?.focus();
  }

  return (
    <View style={styles.container}>
      <View style={styles.inputRow}>
        <TextInput
          ref={inputRef}
          style={styles.input}
          value={text}
          onChangeText={setText}
          onSubmitEditing={handleSubmit}
          placeholder="Type here..."
          placeholderTextColor="#484f58"
          autoCapitalize="none"
          autoCorrect={false}
          autoComplete="off"
          spellCheck={false}
          returnKeyType="send"
          blurOnSubmit={false}
        />
        <TouchableOpacity style={styles.sendBtn} onPress={handleSubmit}>
          <Text style={styles.sendText}>⏎</Text>
        </TouchableOpacity>
      </View>
      <ScrollView
        horizontal
        style={styles.quickBar}
        contentContainerStyle={styles.quickContent}
        showsHorizontalScrollIndicator={false}
        keyboardShouldPersistTaps="always"
      >
        {QUICK_COMMANDS.map((cmd) => (
          <TouchableOpacity
            key={cmd.label}
            style={[styles.quickBtn, { borderColor: cmd.color }]}
            onPress={() => handleQuickCommand(cmd.command)}
          >
            <Text style={[styles.quickText, { color: cmd.color }]}>{cmd.label}</Text>
          </TouchableOpacity>
        ))}
      </ScrollView>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    backgroundColor: '#161b22',
    borderTopWidth: 1,
    borderTopColor: '#30363d',
  },
  inputRow: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: 8,
    paddingTop: 6,
    paddingBottom: 4,
    gap: 6,
  },
  input: {
    flex: 1,
    backgroundColor: '#0d1117',
    borderWidth: 1,
    borderColor: '#30363d',
    borderRadius: 6,
    paddingHorizontal: 10,
    paddingVertical: 6,
    color: '#c9d1d9',
    fontSize: 14,
    fontFamily: Platform.select({ ios: 'Menlo', android: 'monospace' }),
  },
  sendBtn: {
    backgroundColor: '#238636',
    borderRadius: 6,
    paddingHorizontal: 12,
    paddingVertical: 6,
  },
  sendText: {
    color: '#fff',
    fontSize: 16,
  },
  quickBar: {
    maxHeight: 36,
    paddingBottom: 6,
  },
  quickContent: {
    alignItems: 'center',
    paddingHorizontal: 8,
    gap: 6,
  },
  quickBtn: {
    paddingHorizontal: 10,
    paddingVertical: 4,
    borderRadius: 5,
    backgroundColor: '#21262d',
    borderWidth: 1,
  },
  quickText: {
    fontSize: 11,
    fontFamily: Platform.select({ ios: 'Menlo', android: 'monospace' }),
  },
});
