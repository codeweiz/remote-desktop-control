import { useState, useEffect, useCallback } from 'react';
import AsyncStorage from '@react-native-async-storage/async-storage';

const STORAGE_KEY = 'rtb-terminal-settings';

export interface TerminalSettings {
  fontSize: number;
  theme: 'dark' | 'monokai' | 'solarized' | 'light';
}

const DEFAULTS: TerminalSettings = { fontSize: 14, theme: 'dark' };

export const THEME_LABELS: Record<string, string> = {
  dark: 'Dark',
  monokai: 'Monokai',
  solarized: 'Solarized',
  light: 'Light',
};

export const THEME_COLORS: Record<string, { background: string; foreground: string; cursor: string }> = {
  dark: { background: '#1e1e1e', foreground: '#d4d4d4', cursor: '#58a6ff' },
  monokai: { background: '#272822', foreground: '#f8f8f2', cursor: '#f8f8f0' },
  solarized: { background: '#002b36', foreground: '#839496', cursor: '#93a1a1' },
  light: { background: '#ffffff', foreground: '#383a42', cursor: '#526eff' },
};

export function useTerminalSettings() {
  const [settings, setSettings] = useState<TerminalSettings>(DEFAULTS);

  useEffect(() => {
    AsyncStorage.getItem(STORAGE_KEY).then((raw) => {
      if (raw) {
        try {
          const parsed = JSON.parse(raw);
          if (parsed && typeof parsed.fontSize === 'number' && THEME_COLORS[parsed.theme]) {
            setSettings(parsed);
          }
        } catch {}
      }
    });
  }, []);

  const update = useCallback(async (partial: Partial<TerminalSettings>) => {
    setSettings(prev => {
      const next = { ...prev, ...partial };
      AsyncStorage.setItem(STORAGE_KEY, JSON.stringify(next));
      return next;
    });
  }, []);

  return { settings, update };
}
