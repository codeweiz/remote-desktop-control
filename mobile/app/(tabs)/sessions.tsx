import { View, Text, StyleSheet } from 'react-native';

export default function SessionsScreen() {
  return (
    <View style={styles.container}>
      <Text style={styles.text}>Sessions</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: '#0d1117', justifyContent: 'center', alignItems: 'center' },
  text: { color: '#c9d1d9', fontSize: 18 },
});
