import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';

type Capabilities = { auth_required: boolean; mode: string };

type LoginResponse = {
  token: string;
  user: {
    id: number;
    username: string;
  };
};

function serverUrl(): string {
  const cfg = vscode.workspace.getConfiguration('racehub');
  const raw = cfg.get<string>('serverUrl') ?? 'http://127.0.0.1:8787';
  return raw.replace(/\/$/, '');
}

async function setServerUrl(): Promise<void> {
  const current = serverUrl();
  const value = await vscode.window.showInputBox({
    title: 'RaceHub Server URL',
    value: current,
    prompt: 'Example: http://127.0.0.1:8787'
  });
  if (!value) {
    return;
  }
  await vscode.workspace.getConfiguration('racehub').update('serverUrl', value, vscode.ConfigurationTarget.Global);
  void vscode.window.showInformationMessage(`RaceHub server URL set to ${value}`);
}

async function fetchCapabilities(): Promise<Capabilities> {
  const resp = await fetch(`${serverUrl()}/api/v1/capabilities`);
  if (!resp.ok) {
    throw new Error(`capabilities failed: ${resp.status} ${resp.statusText}`);
  }
  return await resp.json() as Capabilities;
}

async function login(context: vscode.ExtensionContext): Promise<void> {
  const username = await vscode.window.showInputBox({ title: 'RaceHub Username' });
  if (!username) {
    return;
  }
  const password = await vscode.window.showInputBox({ title: 'RaceHub Password', password: true });
  if (!password) {
    return;
  }

  const resp = await fetch(`${serverUrl()}/api/v1/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password })
  });

  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(`login failed: ${resp.status} ${text}`);
  }

  const data = await resp.json() as LoginResponse;
  await context.secrets.store('racehub.token', data.token);
  void vscode.window.showInformationMessage(`RaceHub login successful as ${data.user.username}`);
}

async function getTokenForMode(context: vscode.ExtensionContext, caps: Capabilities): Promise<string | undefined> {
  if (!caps.auth_required) {
    return undefined;
  }

  let token = await context.secrets.get('racehub.token');
  if (!token) {
    await login(context);
    token = await context.secrets.get('racehub.token') ?? undefined;
  }
  if (!token) {
    throw new Error('missing auth token');
  }
  return token;
}

async function uploadArtifact(context: vscode.ExtensionContext): Promise<void> {
  const caps = await fetchCapabilities();
  const token = await getTokenForMode(context, caps);

  const picked = await vscode.window.showOpenDialog({
    canSelectMany: false,
    openLabel: 'Select ELF Artifact',
  });

  if (!picked || picked.length === 0) {
    return;
  }

  const filePath = picked[0].fsPath;
  const fileName = path.basename(filePath);

  const name = await vscode.window.showInputBox({
    title: 'Artifact Name',
    value: fileName
  });

  if (!name) {
    return;
  }

  const note = await vscode.window.showInputBox({
    title: 'Artifact Note (optional)',
    value: ''
  });

  const target = await vscode.window.showInputBox({
    title: 'Target Triple',
    value: 'riscv32imafc-unknown-none-elf'
  });

  if (!target) {
    return;
  }

  const bytes = fs.readFileSync(filePath);
  const elfBase64 = bytes.toString('base64');

  const headers: Record<string, string> = {
    'Content-Type': 'application/json'
  };

  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  const resp = await fetch(`${serverUrl()}/api/v1/artifacts`, {
    method: 'POST',
    headers,
    body: JSON.stringify({
      name,
      note: note && note.trim().length > 0 ? note.trim() : null,
      target,
      elf_base64: elfBase64
    })
  });

  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(`artifact upload failed: ${resp.status} ${text}`);
  }

  const data = await resp.json() as { artifact_id: number };
  void vscode.window.showInformationMessage(`Artifact uploaded: #${data.artifact_id}`);
}

export function activate(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand('racehub.configureServer', async () => {
      try {
        await setServerUrl();
      } catch (error) {
        void vscode.window.showErrorMessage(String(error));
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('racehub.login', async () => {
      try {
        await login(context);
      } catch (error) {
        void vscode.window.showErrorMessage(String(error));
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('racehub.uploadArtifact', async () => {
      try {
        await uploadArtifact(context);
      } catch (error) {
        void vscode.window.showErrorMessage(String(error));
      }
    })
  );
}

export function deactivate(): void {}
