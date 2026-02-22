import * as vscode from 'vscode';

import { loginRequest } from '../api';
import { resolveServerUrl } from '../config';

const TOKEN_KEY = 'racehub.token';

function html(serverUrl: string): string {
  const registerUrl = `${serverUrl}/register`;
  return `<!doctype html>
<html>
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <style>
    body { font-family: var(--vscode-font-family); padding: 16px; color: var(--vscode-foreground); }
    .wrap { max-width: 420px; }
    h2 { margin-top: 0; }
    .field { margin-bottom: 12px; }
    label { display: block; margin-bottom: 6px; }
    input { width: 100%; box-sizing: border-box; padding: 8px; background: var(--vscode-input-background); color: var(--vscode-input-foreground); border: 1px solid var(--vscode-input-border); }
    button { padding: 8px 12px; }
    .error { margin-top: 10px; color: var(--vscode-errorForeground); min-height: 1.2em; }
    .meta { margin-bottom: 14px; opacity: 0.85; }
    a { color: var(--vscode-textLink-foreground); }
  </style>
</head>
<body>
  <div class="wrap">
    <h2>RaceHub Login</h2>
    <div class="meta">Server: <code>${serverUrl}</code></div>
    <div class="field">
      <label for="username">Username</label>
      <input id="username" type="text" autocomplete="username" />
    </div>
    <div class="field">
      <label for="password">Password</label>
      <input id="password" type="password" autocomplete="current-password" />
    </div>
    <button id="login">Log in</button>
    <div class="error" id="error"></div>
    <p><a href="${registerUrl}" target="_blank" rel="noreferrer">Create account</a></p>
  </div>
  <script>
    const vscode = acquireVsCodeApi();
    const loginBtn = document.getElementById('login');
    const usernameEl = document.getElementById('username');
    const passwordEl = document.getElementById('password');
    const errorEl = document.getElementById('error');

    function submit() {
      errorEl.textContent = '';
      vscode.postMessage({
        type: 'login',
        username: usernameEl.value,
        password: passwordEl.value,
      });
    }

    loginBtn.addEventListener('click', submit);
    passwordEl.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        submit();
      }
    });

    window.addEventListener('message', (event) => {
      const msg = event.data;
      if (msg.type === 'error') {
        errorEl.textContent = msg.message;
      }
    });
  </script>
</body>
</html>`;
}

export async function openLoginWebview(context: vscode.ExtensionContext): Promise<boolean> {
  const serverUrl = resolveServerUrl();
  const panel = vscode.window.createWebviewPanel('racehub.login', 'RaceHub Login', vscode.ViewColumn.Active, {
    enableScripts: true
  });

  panel.webview.html = html(serverUrl);

  return await new Promise<boolean>((resolve) => {
    const messageSubscription = panel.webview.onDidReceiveMessage(async (message) => {
      if (message?.type !== 'login') {
        return;
      }

      const username = String(message.username ?? '').trim();
      const password = String(message.password ?? '');
      if (!username || !password) {
        panel.webview.postMessage({ type: 'error', message: 'Username and password are required.' });
        return;
      }

      try {
        const resp = await loginRequest(username, password);
        await context.secrets.store(TOKEN_KEY, resp.token);
        void vscode.window.showInformationMessage(`RaceHub login successful as ${resp.user.username}`);
        panel.dispose();
        resolve(true);
      } catch (error) {
        panel.webview.postMessage({ type: 'error', message: String(error) });
      }
    });

    const disposeSubscription = panel.onDidDispose(() => {
      messageSubscription.dispose();
      disposeSubscription.dispose();
      resolve(false);
    });
  });
}
