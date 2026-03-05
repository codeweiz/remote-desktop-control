import { useRef, useEffect, useCallback, useState } from 'react';
import { View, Text, StyleSheet, Platform } from 'react-native';
import { WebView } from 'react-native-webview';
import { useLocalSearchParams, useFocusEffect } from 'expo-router';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import * as Notifications from 'expo-notifications';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { useServer } from '../../hooks/useServer';
import { THEME_COLORS } from '../../hooks/useTerminalSettings';
import { QuickCommandBar } from '../../components/QuickCommandBar';

const TERMINAL_HTML = `<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8"/>
<meta name="viewport" content="width=device-width,initial-scale=1,maximum-scale=1,user-scalable=no"/>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@xterm/xterm@5/css/xterm.css"/>
<style>
  *{margin:0;padding:0;box-sizing:border-box;}
  html,body{height:100%;overflow:hidden;background:#1e1e1e;}
  #terminal{width:100%;height:100%;}
</style>
</head>
<body>
<div id="terminal"></div>
<script type="module">
import {Terminal} from 'https://cdn.jsdelivr.net/npm/@xterm/xterm@5/+esm';
import {FitAddon} from 'https://cdn.jsdelivr.net/npm/@xterm/addon-fit@0/+esm';

const term = new Terminal({
  cursorBlink: true, fontSize: 14,
  fontFamily: '"SF Mono",Menlo,Monaco,monospace',
  theme: { background:'#1e1e1e', foreground:'#d4d4d4', cursor:'#58a6ff' },
  allowProposedApi: true,
});
const fitAddon = new FitAddon();
term.loadAddon(fitAddon);
term.open(document.getElementById('terminal'));
fitAddon.fit();

// Focus terminal textarea on tap so keyboard appears
document.getElementById('terminal').addEventListener('click', () => {
  const textarea = document.querySelector('.xterm-helper-textarea');
  if (textarea) {
    textarea.focus();
  }
});

let ws = null;

window.addEventListener('message', (e) => {
  try {
    const msg = JSON.parse(e.data);
    if (msg.type === 'connect') {
      connectWS(msg.wsUrl, msg.sessionId, msg.token);
    } else if (msg.type === 'input') {
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'input', data: msg.data }));
      }
    } else if (msg.type === 'buffer') {
      if (msg.data) term.write(msg.data);
    } else if (msg.type === 'settings') {
      if (msg.fontSize) term.options.fontSize = msg.fontSize;
      if (msg.theme) {
        term.options.theme = { ...term.options.theme, ...msg.theme };
        document.body.style.background = msg.theme.background || '#1e1e1e';
      }
      fitAddon.fit();
      // Report new size after settings change
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'resize', cols: term.cols, rows: term.rows }));
      }
    } else if (msg.type === 'focus') {
      const textarea = document.querySelector('.xterm-helper-textarea');
      if (textarea) textarea.focus();
    }
  } catch {}
});

term.onData((data) => {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'input', data }));
  }
});

function connectWS(wsUrl, sessionId, token) {
  if (ws) { ws.onclose = null; ws.close(); }
  term.clear(); term.reset();

  ws = new WebSocket(wsUrl + '/ws/' + sessionId + '?token=' + token);
  ws.onopen = () => {
    window.ReactNativeWebView?.postMessage(JSON.stringify({ type: 'connected' }));
    ws.send(JSON.stringify({ type: 'resize', cols: term.cols, rows: term.rows }));
  };
  ws.onmessage = (event) => {
    try {
      const msg = JSON.parse(event.data);
      if (msg.type === 'output') term.write(msg.data);
      if (msg.type === 'notification') {
        window.ReactNativeWebView?.postMessage(JSON.stringify({ type: 'notification', message: msg.message }));
      }
    } catch {}
  };
  ws.onclose = () => {
    window.ReactNativeWebView?.postMessage(JSON.stringify({ type: 'disconnected' }));
  };
}

window.addEventListener('resize', () => fitAddon.fit());
term.onResize(({cols, rows}) => {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'resize', cols, rows }));
  }
});
<\/script>
</body>
</html>`;

async function loadSettings() {
  try {
    const raw = await AsyncStorage.getItem('rtb-terminal-settings');
    if (raw) {
      const parsed = JSON.parse(raw);
      if (parsed && typeof parsed.fontSize === 'number' && THEME_COLORS[parsed.theme]) {
        return parsed;
      }
    }
  } catch {}
  return { fontSize: 14, theme: 'dark' };
}

export default function TerminalScreen() {
  const { sessionId } = useLocalSearchParams<{ sessionId?: string }>();
  const { wsUrl, baseUrl, config } = useServer();
  const insets = useSafeAreaInsets();
  const webViewRef = useRef<WebView>(null);
  const [webViewReady, setWebViewReady] = useState(false);

  const sendToWebView = useCallback((msg: object) => {
    webViewRef.current?.injectJavaScript(
      `window.postMessage(${JSON.stringify(JSON.stringify(msg))}); true;`
    );
  }, []);

  // Apply settings from AsyncStorage whenever terminal tab gains focus
  useFocusEffect(
    useCallback(() => {
      if (!webViewReady) return;
      loadSettings().then((s) => {
        const colors = THEME_COLORS[s.theme] || THEME_COLORS.dark;
        sendToWebView({ type: 'settings', fontSize: s.fontSize, theme: colors });
      });
    }, [webViewReady, sendToWebView])
  );

  useEffect(() => {
    if (sessionId && wsUrl && config && webViewReady) {
      const timer = setTimeout(async () => {
        // Apply saved settings first
        const s = await loadSettings();
        const colors = THEME_COLORS[s.theme] || THEME_COLORS.dark;
        sendToWebView({ type: 'settings', fontSize: s.fontSize, theme: colors });

        // Then fetch buffer and connect
        try {
          const res = await fetch(`${baseUrl}/api/sessions/buffer?id=${sessionId}`);
          const { buffer } = await res.json();
          if (buffer) sendToWebView({ type: 'buffer', data: buffer });
        } catch {}
        sendToWebView({ type: 'connect', wsUrl, sessionId, token: config.token });
      }, 500);
      return () => clearTimeout(timer);
    }
  }, [sessionId, wsUrl, config, baseUrl, sendToWebView, webViewReady]);

  function handleCommand(command: string) {
    sendToWebView({ type: 'input', data: command });
  }

  if (!sessionId) {
    return (
      <View style={[styles.placeholder, { paddingTop: insets.top }]}>
        <Text style={styles.placeholderText}>Select a session to connect</Text>
      </View>
    );
  }

  return (
    <View style={[styles.container, { paddingTop: insets.top }]}>
      <WebView
        ref={webViewRef}
        source={{ html: TERMINAL_HTML, baseUrl: 'https://cdn.jsdelivr.net' }}
        style={styles.webview}
        javaScriptEnabled
        originWhitelist={['*']}
        allowsInlineMediaPlayback
        mixedContentMode="always"
        keyboardDisplayRequiresUserAction={false}
        onLoad={() => setWebViewReady(true)}
        onMessage={(event) => {
          try {
            const msg = JSON.parse(event.nativeEvent.data);
            if (msg.type === 'notification') {
              Notifications.scheduleNotificationAsync({
                content: {
                  title: 'RTB',
                  body: msg.message || 'Terminal needs attention',
                },
                trigger: null,
              });
            }
          } catch {}
        }}
      />
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
