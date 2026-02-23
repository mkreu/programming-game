import * as vscode from 'vscode';

import { initializeBotProject, openBotProject } from './bootstrap';
import { loginViaWebview } from './auth';
import { configureServerProfile } from './config';
import { RaceHubItem, RaceHubViewProvider } from './views/racehubView';

function registerCommand(
  context: vscode.ExtensionContext,
  command: string,
  fn: (...args: any[]) => Promise<void>
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(command, async (...args: any[]) => {
      try {
        await fn(...args);
      } catch (error) {
        void vscode.window.showErrorMessage(String(error));
      }
    })
  );
}

export function activate(context: vscode.ExtensionContext): void {
  const provider = new RaceHubViewProvider(context);

  const view = vscode.window.createTreeView('racehub.explorer', {
    treeDataProvider: provider,
    showCollapseAll: true
  });
  context.subscriptions.push(view);

  const refresh = (): void => {
    void provider.refreshArtifacts();
  };

  context.subscriptions.push(
    vscode.workspace.onDidChangeWorkspaceFolders(refresh),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration('racehub')) {
        refresh();
      }
    })
  );

  const cargoWatcher = vscode.workspace.createFileSystemWatcher('**/Cargo.toml');
  const binWatcher = vscode.workspace.createFileSystemWatcher('**/src/bin/*.rs');
  context.subscriptions.push(
    cargoWatcher,
    binWatcher,
    cargoWatcher.onDidChange(refresh),
    cargoWatcher.onDidCreate(refresh),
    cargoWatcher.onDidDelete(refresh),
    binWatcher.onDidChange(refresh),
    binWatcher.onDidCreate(refresh),
    binWatcher.onDidDelete(refresh)
  );

  registerCommand(context, 'racehub.configureServer', async () => {
    await configureServerProfile();
    await provider.refreshArtifacts();
  });

  registerCommand(context, 'racehub.login', async () => {
    const changed = await loginViaWebview(context);
    if (changed) {
      await provider.refreshArtifacts();
    }
  });

  registerCommand(context, 'racehub.initializeBotProject', async () => {
    await initializeBotProject(context);
    await provider.refreshArtifacts();
  });

  registerCommand(context, 'racehub.openBotProject', async () => {
    await openBotProject();
  });

  registerCommand(context, 'racehub.view.refresh', async () => {
    await provider.refreshArtifacts();
  });

  registerCommand(context, 'racehub.view.buildAndUpload', async (item?: RaceHubItem) => {
    await provider.buildAndUploadBinary(item);
  });

  registerCommand(context, 'racehub.view.buildBinary', async (item?: RaceHubItem) => {
    await provider.buildBinaryItem(item);
  });

  registerCommand(context, 'racehub.view.revealElfPath', async (item?: RaceHubItem) => {
    await provider.revealElfPath(item);
  });

  registerCommand(context, 'racehub.view.replaceArtifact', async (item?: RaceHubItem) => {
    await provider.replaceArtifact(item);
  });

  registerCommand(context, 'racehub.view.deleteArtifact', async (item?: RaceHubItem) => {
    await provider.deleteArtifact(item);
  });

  registerCommand(context, 'racehub.view.toggleVisibility', async (item?: RaceHubItem) => {
    await provider.toggleVisibility(item);
  });

  refresh();
}

export function deactivate(): void {}
