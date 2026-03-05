import { useState, useEffect, useCallback } from 'react';
import AsyncStorage from '@react-native-async-storage/async-storage';

const STORAGE_KEY = 'rtb-server-config';

export interface ServerConfig {
  address: string; // e.g. "rtb.micro-boat.com" or "192.168.1.100:3000"
  token: string;
  useSSL: boolean;
}

export function useServer() {
  const [config, setConfig] = useState<ServerConfig | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    AsyncStorage.getItem(STORAGE_KEY).then((raw) => {
      if (raw) setConfig(JSON.parse(raw));
      setLoading(false);
    });
  }, []);

  const save = useCallback(async (newConfig: ServerConfig) => {
    await AsyncStorage.setItem(STORAGE_KEY, JSON.stringify(newConfig));
    setConfig(newConfig);
  }, []);

  const clear = useCallback(async () => {
    await AsyncStorage.removeItem(STORAGE_KEY);
    setConfig(null);
  }, []);

  const baseUrl = config
    ? `${config.useSSL ? 'https' : 'http'}://${config.address}`
    : null;

  const wsUrl = config
    ? `${config.useSSL ? 'wss' : 'ws'}://${config.address}`
    : null;

  return { config, loading, save, clear, baseUrl, wsUrl };
}
