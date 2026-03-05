import { useState, useEffect, useCallback } from 'react';
import { useColorScheme } from 'react-native';
import AsyncStorage from '@react-native-async-storage/async-storage';

export type AppThemeMode = 'light' | 'dark' | 'system';

const STORAGE_KEY = 'rtb-app-theme';

export const LIGHT_COLORS = {
  background: '#ffffff',
  surface: '#f6f8fa',
  surfaceBorder: '#d0d7de',
  headerBg: '#f6f8fa',
  headerText: '#1f2328',
  tabBarBg: '#f6f8fa',
  tabBarBorder: '#d0d7de',
  tabBarActive: '#0969da',
  tabBarInactive: '#656d76',
  text: '#1f2328',
  textSecondary: '#656d76',
  textMuted: '#8c959f',
  cardBg: '#ffffff',
  cardBorder: '#d0d7de',
  inputBg: '#f6f8fa',
  inputBorder: '#d0d7de',
  accent: '#0969da',
  danger: '#cf222e',
  dangerBg: '#cf222e1a',
  success: '#1a7f37',
  searchBg: '#f6f8fa',
  searchBorder: '#d0d7de',
  fabBg: '#1a7f37',
  overlayBg: 'rgba(0,0,0,.4)',
  terminalBg: '#ffffff',
};

export const DARK_COLORS = {
  background: '#0d1117',
  surface: '#161b22',
  surfaceBorder: '#30363d',
  headerBg: '#161b22',
  headerText: '#f0f6fc',
  tabBarBg: '#161b22',
  tabBarBorder: '#30363d',
  tabBarActive: '#58a6ff',
  tabBarInactive: '#8b949e',
  text: '#c9d1d9',
  textSecondary: '#8b949e',
  textMuted: '#484f58',
  cardBg: '#161b22',
  cardBorder: '#30363d',
  inputBg: '#0d1117',
  inputBorder: '#30363d',
  accent: '#58a6ff',
  danger: '#f85149',
  dangerBg: '#da36331a',
  success: '#238636',
  searchBg: '#161b22',
  searchBorder: '#30363d',
  fabBg: '#238636',
  overlayBg: 'rgba(0,0,0,.6)',
  terminalBg: '#0d1117',
};

export type AppColors = typeof DARK_COLORS;

export function useAppTheme() {
  const systemScheme = useColorScheme();
  const [mode, setMode] = useState<AppThemeMode>('dark');
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    AsyncStorage.getItem(STORAGE_KEY).then((val) => {
      if (val === 'light' || val === 'dark' || val === 'system') {
        setMode(val);
      }
      setLoaded(true);
    });
  }, []);

  const updateMode = useCallback(async (newMode: AppThemeMode) => {
    setMode(newMode);
    await AsyncStorage.setItem(STORAGE_KEY, newMode);
  }, []);

  const resolvedScheme = mode === 'system' ? (systemScheme || 'dark') : mode;
  const colors: AppColors = resolvedScheme === 'light' ? LIGHT_COLORS : DARK_COLORS;

  return { mode, updateMode, colors, isDark: resolvedScheme === 'dark', loaded };
}
