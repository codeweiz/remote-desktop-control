import { useRef, useEffect, useCallback, useState } from 'react';
import { View, Text, StyleSheet, AppState, TouchableOpacity } from 'react-native';
import { WebView } from 'react-native-webview';
import { useLocalSearchParams, useFocusEffect } from 'expo-router';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { useServer } from '../../hooks/useServer';
import { THEME_COLORS } from '../../hooks/useTerminalSettings';
import { useTheme } from '../../contexts/ThemeContext';
import { TerminalInput } from '../../components/TerminalInput';

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

let ws = null;
let reconnectTimer = null;
let lastConnectParams = null;
let reconnectAttempts = 0;

function notify(type, extra) {
  window.ReactNativeWebView?.postMessage(JSON.stringify({ type, ...extra }));
}

window.addEventListener('message', (e) => {
  try {
    const msg = JSON.parse(e.data);
    if (msg.type === 'connect') {
      lastConnectParams = { wsUrl: msg.wsUrl, sessionId: msg.sessionId, token: msg.token };
      reconnectAttempts = 0;
      connectWS(msg.wsUrl, msg.sessionId, msg.token, false);
    } else if (msg.type === 'reconnect') {
      if (lastConnectParams) {
        reconnectAttempts = 0;
        connectWS(lastConnectParams.wsUrl, lastConnectParams.sessionId, lastConnectParams.token, true);
      }
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
      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'resize', cols: term.cols, rows: term.rows }));
      }
    }
  } catch {}
});

term.onData((data) => {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'input', data }));
  }
});

function connectWS(wsUrl, sessionId, token, isReconnect) {
  clearTimeout(reconnectTimer);
  if (ws) { ws.onclose = null; ws.onerror = null; ws.close(); }
  if (!isReconnect) { term.clear(); term.reset(); }

  notify('connecting');
  ws = new WebSocket(wsUrl + '/ws/' + sessionId + '?token=' + token);

  ws.onopen = () => {
    reconnectAttempts = 0;
    notify('connected');
    ws.send(JSON.stringify({ type: 'resize', cols: term.cols, rows: term.rows }));
  };
  ws.onmessage = (event) => {
    try {
      const msg = JSON.parse(event.data);
      if (msg.type === 'output') term.write(msg.data);
      if (msg.type === 'notification') {
        notify('notification', { message: msg.message });
      }
    } catch {}
  };
  ws.onclose = () => {
    notify('disconnected');
    // Auto-reconnect with backoff (max 10s)
    reconnectAttempts++;
    const delay = Math.min(1000 * reconnectAttempts, 10000);
    reconnectTimer = setTimeout(() => {
      if (lastConnectParams) {
        connectWS(lastConnectParams.wsUrl, lastConnectParams.sessionId, lastConnectParams.token, true);
      }
    }, delay);
  };
  ws.onerror = () => {};
}

// Heartbeat
setInterval(() => {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({ type: 'ping' }));
  }
}, 25000);

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

type ConnStatus = 'idle' | 'connecting' | 'connected' | 'disconnected';

export default function TerminalScreen() {
  const { sessionId } = useLocalSearchParams<{ sessionId?: string }>();
  const { wsUrl, baseUrl, config } = useServer();
  const { colors } = useTheme();
  const insets = useSafeAreaInsets();
  const webViewRef = useRef<WebView>(null);
  const [webViewReady, setWebViewReady] = useState(false);
  const [connStatus, setConnStatus] = useState<ConnStatus>('idle');

  const sendToWebView = useCallback((msg: object) => {
    webViewRef.current?.injectJavaScript(
      `window.postMessage(${JSON.stringify(JSON.stringify(msg))}); true;`
    );
  }, []);

  const connectedOnce = useRef(false);

  // Apply settings when tab gains focus; reconnect only after initial connect
  useFocusEffect(
    useCallback(() => {
      if (!webViewReady) return;
      loadSettings().then((s) => {
        const themeColors = THEME_COLORS[s.theme] || THEME_COLORS.dark;
        sendToWebView({ type: 'settings', fontSize: s.fontSize, theme: themeColors });
      });
      if (sessionId && connectedOnce.current) {
        sendToWebView({ type: 'reconnect' });
      }
    }, [webViewReady, sendToWebView, sessionId])
  );

  // Reconnect when app returns from background
  useEffect(() => {
    const sub = AppState.addEventListener('change', (state) => {
      if (state === 'active' && webViewReady && sessionId && connectedOnce.current) {
        sendToWebView({ type: 'reconnect' });
      }
    });
    return () => sub.remove();
  }, [webViewReady, sessionId, sendToWebView]);

  useEffect(() => {
    connectedOnce.current = false;
    setConnStatus('idle');
    if (sessionId && wsUrl && config && webViewReady) {
      const timer = setTimeout(async () => {
        const s = await loadSettings();
        const themeColors = THEME_COLORS[s.theme] || THEME_COLORS.dark;
        sendToWebView({ type: 'settings', fontSize: s.fontSize, theme: themeColors });

        try {
          const res = await fetch(`${baseUrl}/api/sessions/buffer?id=${sessionId}`);
          const { buffer } = await res.json();
          if (buffer) sendToWebView({ type: 'buffer', data: buffer });
        } catch {}
        sendToWebView({ type: 'connect', wsUrl, sessionId, token: config.token });
        connectedOnce.current = true;
      }, 500);
      return () => clearTimeout(timer);
    }
  }, [sessionId, wsUrl, config, baseUrl, sendToWebView, webViewReady]);

  function handleInput(data: string) {
    sendToWebView({ type: 'input', data });
  }

  function handleReconnect() {
    sendToWebView({ type: 'reconnect' });
  }

  if (!sessionId) {
    return (
      <View style={[styles.placeholder, { paddingTop: insets.top, backgroundColor: colors.background }]}>
        <Text style={[styles.placeholderText, { color: colors.textMuted }]}>Select a session to connect</Text>
      </View>
    );
  }

  const statusColor = connStatus === 'connected' ? '#2ea043'
    : connStatus === 'connecting' ? '#d29922'
    : connStatus === 'disconnected' ? '#f85149'
    : '#484f58';

  return (
    <View style={[styles.container, { paddingTop: insets.top, backgroundColor: colors.terminalBg }]}>
      {connStatus !== 'connected' && connStatus !== 'idle' && (
        <TouchableOpacity style={[styles.statusBar, { backgroundColor: statusColor + '22' }]} onPress={handleReconnect}>
          <View style={[styles.statusDot, { backgroundColor: statusColor }]} />
          <Text style={[styles.statusText, { color: statusColor }]}>
            {connStatus === 'connecting' ? 'Connecting...' : 'Disconnected — tap to reconnect'}
          </Text>
        </TouchableOpacity>
      )}
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
            if (msg.type === 'connected') setConnStatus('connected');
            else if (msg.type === 'disconnected') setConnStatus('disconnected');
            else if (msg.type === 'connecting') setConnStatus('connecting');
          } catch {}
        }}
      />
      <TerminalInput onInput={handleInput} />
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1 },
  webview: { flex: 1 },
  placeholder: { flex: 1, justifyContent: 'center', alignItems: 'center' },
  placeholderText: { fontSize: 15 },
  statusBar: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: 12,
    paddingVertical: 6,
    gap: 6,
  },
  statusDot: {
    width: 8,
    height: 8,
    borderRadius: 4,
  },
  statusText: {
    fontSize: 12,
    fontWeight: '500',
  },
});
