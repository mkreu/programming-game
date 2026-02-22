import * as vscode from 'vscode';

export const PRODUCTION_SERVER_URL = 'https://racers.mlkr.eu';
export const LOCALHOST_SERVER_URL = 'http://127.0.0.1:8787';
export const DEFAULT_TARGET_TRIPLE = 'riscv32imafc-unknown-none-elf';

export type ServerProfile = 'production' | 'localhost' | 'custom';

export function defaultArtifactTarget(): string {
  const cfg = vscode.workspace.getConfiguration('racehub');
  return cfg.get<string>('defaultArtifactTarget') ?? DEFAULT_TARGET_TRIPLE;
}

export function configuredBotWorkspacePath(): string | undefined {
  const cfg = vscode.workspace.getConfiguration('racehub');
  const value = cfg.get<string>('botWorkspacePath');
  if (!value) {
    return undefined;
  }
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
}

export function getServerProfile(): ServerProfile {
  const cfg = vscode.workspace.getConfiguration('racehub');
  const profile = cfg.get<string>('serverProfile') ?? 'production';
  if (profile === 'localhost' || profile === 'custom' || profile === 'production') {
    return profile;
  }
  return 'production';
}

export function getCustomServerUrl(): string {
  const cfg = vscode.workspace.getConfiguration('racehub');
  return (cfg.get<string>('customServerUrl') ?? '').trim();
}

export function resolveServerUrl(): string {
  const profile = getServerProfile();
  if (profile === 'production') {
    return PRODUCTION_SERVER_URL;
  }
  if (profile === 'localhost') {
    return LOCALHOST_SERVER_URL;
  }

  const custom = getCustomServerUrl();
  if (custom.length === 0) {
    throw new Error('Custom server profile is selected, but racehub.customServerUrl is empty.');
  }

  try {
    const parsed = new URL(custom);
    return parsed.toString().replace(/\/$/, '');
  } catch {
    throw new Error(`Invalid racehub.customServerUrl: '${custom}'`);
  }
}

async function setServerProfile(profile: ServerProfile): Promise<void> {
  await vscode.workspace.getConfiguration('racehub').update('serverProfile', profile, vscode.ConfigurationTarget.Global);
}

async function setCustomServerUrl(url: string): Promise<void> {
  await vscode.workspace.getConfiguration('racehub').update('customServerUrl', url, vscode.ConfigurationTarget.Global);
}

export async function configureServerProfile(): Promise<void> {
  const picked = await vscode.window.showQuickPick(
    [
      {
        label: 'Production',
        description: PRODUCTION_SERVER_URL,
        profile: 'production' as ServerProfile
      },
      {
        label: 'Localhost',
        description: LOCALHOST_SERVER_URL,
        profile: 'localhost' as ServerProfile
      },
      {
        label: 'Custom...',
        description: 'Enter a custom RaceHub URL',
        profile: 'custom' as ServerProfile
      }
    ],
    {
      title: 'Select RaceHub Server'
    }
  );

  if (!picked) {
    return;
  }

  if (picked.profile === 'custom') {
    const current = getCustomServerUrl();
    const value = await vscode.window.showInputBox({
      title: 'Custom RaceHub URL',
      value: current,
      prompt: 'Example: https://racers.mlkr.eu'
    });

    if (!value) {
      return;
    }

    let normalized: string;
    try {
      normalized = new URL(value).toString().replace(/\/$/, '');
    } catch {
      throw new Error(`Invalid URL: '${value}'`);
    }

    await setCustomServerUrl(normalized);
    await setServerProfile('custom');
    void vscode.window.showInformationMessage(`RaceHub server set to ${normalized}`);
    return;
  }

  await setServerProfile(picked.profile);
  const effective = picked.profile === 'production' ? PRODUCTION_SERVER_URL : LOCALHOST_SERVER_URL;
  void vscode.window.showInformationMessage(`RaceHub server set to ${effective}`);
}
