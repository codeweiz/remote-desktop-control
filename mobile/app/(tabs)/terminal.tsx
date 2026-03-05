import { useRef, useEffect, useCallback } from 'react';
import { View, Text, StyleSheet } from 'react-native';
import { WebView } from 'react-native-webview';
import { useLocalSearchParams } from 'expo-router';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import * as Notifications from 'expo-notifications';
import { useServer } from '../../hooks/useServer';
import { useTerminalSettings, THEME_COLORS } from '../../hooks/useTerminalSettings';
import { QuickCommandBar } from '../../components/QuickCommandBar';

function buildTerminalHtml(fontSize: number, theme: string) {
  const colors = THEME_COLORS[theme] || THEME_COLORS.dark;
  return `<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8"/>
<meta name="viewport" content="width=device-width,initial-scale=1,maximum-scale=1,user-scalable=no"/>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@xterm/xterm@5/css/xterm.css"/>
<style>
  *{margin:0;padding:0;box-sizing:border-box;}
  html,body{height:100%;overflow:hidden;background:${colors.background};}
  #terminal{width:100%;height:100%;}
</style>
</head>
<body>
<div id="terminal"></div>
<script type="module">
import {Terminal} from 'https://cdn.jsdelivr.net/npm/@xterm/xterm@5/+esm';
import {FitAddon} from 'https://cdn.jsdelivr.net/npm/@xterm/addon-fit@0/+esm';

const term = new Terminal({
  cursorBlink: true, fontSize: ${fontSize},
  fontFamily: '"SF Mono",Menlo,Monaco,monospace',
  theme: { background:'${colors.background}', foreground:'${colors.foreground}', cursor:'${colors.cursor}' },
  allowProposedApi: true,
});
const fitAddon = new FitAddon();
term.loadAddon(fitAddon);
term.open(document.getElementById('terminal'));
fitAddon.fit();

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
}

export default function TerminalScreen() {
  const { sessionId } = useLocalSearchParams<{ sessionId?: string }>();
  const { wsUrl, baseUrl, config } = useServer();
  const { settings } = useTerminalSettings();
  const insets = useSafeAreaInsets();
  const webViewRef = useRef<WebView>(null);

  const sendToWebView = useCallback((msg: object) => {
    webViewRef.current?.injectJavaScript(
      `window.postMessage(${JSON.stringify(JSON.stringify(msg))}); true;`
    );
  }, []);

  useEffect(() => {
    if (sessionId && wsUrl && config) {
      const timer = setTimeout(async () => {
        try {
          const res = await fetch(`${baseUrl}/api/sessions/buffer?id=${sessionId}`);
          const { buffer } = await res.json();
          if (buffer) sendToWebView({ type: 'buffer', data: buffer });
        } catch {}
        sendToWebView({ type: 'connect', wsUrl, sessionId, token: config.token });
      }, 1000);
      return () => clearTimeout(timer);
    }
  }, [sessionId, wsUrl, config, baseUrl, sendToWebView]);

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
        source={{ html: buildTerminalHtml(settings.fontSize, settings.theme), baseUrl: 'https://cdn.jsdelivr.net' }}
        style={styles.webview}
        javaScriptEnabled
        originWhitelist={['*']}
        allowsInlineMediaPlayback
        mixedContentMode="always"
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
