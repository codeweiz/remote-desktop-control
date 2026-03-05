import { useRef, useState, useEffect, useCallback } from 'react';
import { View, Text, StyleSheet } from 'react-native';
import { WebView } from 'react-native-webview';
import { useLocalSearchParams } from 'expo-router';
import { Asset } from 'expo-asset';
import { useServer } from '../../hooks/useServer';
import { QuickCommandBar } from '../../components/QuickCommandBar';

export default function TerminalScreen() {
  const { sessionId } = useLocalSearchParams<{ sessionId?: string }>();
  const { wsUrl, baseUrl, config } = useServer();
  const webViewRef = useRef<WebView>(null);
  const [htmlUri, setHtmlUri] = useState<string | null>(null);

  useEffect(() => {
    Asset.fromModule(require('../../web/terminal.html'))
      .downloadAsync()
      .then((asset) => {
        setHtmlUri(asset.localUri);
      });
  }, []);

  const sendToWebView = useCallback((msg: object) => {
    webViewRef.current?.injectJavaScript(
      `window.postMessage(${JSON.stringify(JSON.stringify(msg))}); true;`
    );
  }, []);

  useEffect(() => {
    if (sessionId && wsUrl && config) {
      // Fetch buffer first, then connect
      (async () => {
        try {
          const res = await fetch(`${baseUrl}/api/sessions/buffer?id=${sessionId}`);
          const { buffer } = await res.json();
          if (buffer) sendToWebView({ type: 'buffer', data: buffer });
        } catch {}
        sendToWebView({ type: 'connect', wsUrl, sessionId, token: config.token });
      })();
    }
  }, [sessionId, wsUrl, config, baseUrl, sendToWebView]);

  function handleCommand(command: string) {
    sendToWebView({ type: 'input', data: command });
  }

  if (!sessionId) {
    return (
      <View style={styles.placeholder}>
        <Text style={styles.placeholderText}>Select a session to connect</Text>
      </View>
    );
  }

  return (
    <View style={styles.container}>
      {htmlUri && (
        <WebView
          ref={webViewRef}
          source={{ uri: htmlUri }}
          style={styles.webview}
          javaScriptEnabled
          originWhitelist={['*']}
          onMessage={(event) => {
            try {
              const msg = JSON.parse(event.nativeEvent.data);
              // Handle messages from WebView (notifications handled in Task 8)
              void msg;
            } catch {}
          }}
        />
      )}
      <QuickCommandBar onCommand={handleCommand} />
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: '#0d1117' },
  webview: { flex: 1 },
  placeholder: { flex: 1, backgroundColor: '#0d1117', justifyContent: 'center', alignItems: 'center' },
  placeholderText: { color: '#484f58', fontSize: 15 },
});
