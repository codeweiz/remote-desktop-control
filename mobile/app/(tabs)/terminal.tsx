import { View, Text, StyleSheet } from 'react-native';
import { useLocalSearchParams } from 'expo-router';

export default function TerminalScreen() {
  const { sessionId } = useLocalSearchParams<{ sessionId?: string }>();

  return (
    <View style={styles.container}>
      <Text style={styles.text}>
        {sessionId ? `Terminal: ${sessionId}` : 'Select a session'}
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: '#0d1117', justifyContent: 'center', alignItems: 'center' },
  text: { color: '#c9d1d9', fontSize: 18 },
});
