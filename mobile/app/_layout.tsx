import { useEffect } from 'react';
import { LogBox } from 'react-native';
import { Stack } from 'expo-router';
import * as Notifications from 'expo-notifications';
import { ThemeProvider } from '../contexts/ThemeContext';

LogBox.ignoreLogs(['expo-notifications']);

Notifications.setNotificationHandler({
  handleNotification: async () => ({
    shouldShowAlert: true,
    shouldPlaySound: true,
    shouldSetBadge: false,
    shouldShowBanner: true,
    shouldShowList: true,
  }),
});

export default function RootLayout() {
  useEffect(() => {
    Notifications.requestPermissionsAsync();
  }, []);

  return (
    <ThemeProvider>
      <Stack screenOptions={{ headerShown: false }}>
        <Stack.Screen name="connect" />
        <Stack.Screen name="(tabs)" />
      </Stack>
    </ThemeProvider>
  );
}
