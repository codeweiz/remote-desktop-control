import { Tabs } from 'expo-router';

export default function TabsLayout() {
  return (
    <Tabs
      screenOptions={{
        headerShown: false,
        tabBarStyle: { backgroundColor: '#161b22', borderTopColor: '#30363d' },
        tabBarActiveTintColor: '#58a6ff',
        tabBarInactiveTintColor: '#8b949e',
      }}
    >
      <Tabs.Screen
        name="sessions"
        options={{ title: 'Sessions' }}
      />
      <Tabs.Screen
        name="terminal"
        options={{ title: 'Terminal', tabBarLabel: 'Terminal' }}
      />
      <Tabs.Screen
        name="settings"
        options={{ title: 'Settings' }}
      />
    </Tabs>
  );
}
