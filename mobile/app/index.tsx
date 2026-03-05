import { Redirect } from 'expo-router';
import { ActivityIndicator, View } from 'react-native';
import { useServer } from '../hooks/useServer';

export default function Index() {
  const { config, loading } = useServer();

  if (loading) {
    return (
      <View style={{ flex: 1, backgroundColor: '#0d1117', justifyContent: 'center', alignItems: 'center' }}>
        <ActivityIndicator size="large" color="#238636" />
      </View>
    );
  }

  if (config) {
    return <Redirect href="/(tabs)/sessions" />;
  }

  return <Redirect href="/connect" />;
}
