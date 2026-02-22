import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';

function copyRecursive(src: string, dest: string): void {
  const stat = fs.statSync(src);
  if (stat.isDirectory()) {
    fs.mkdirSync(dest, { recursive: true });
    for (const entry of fs.readdirSync(src)) {
      copyRecursive(path.join(src, entry), path.join(dest, entry));
    }
    return;
  }

  fs.mkdirSync(path.dirname(dest), { recursive: true });
  fs.copyFileSync(src, dest);
}

function hasFiles(dir: string): boolean {
  if (!fs.existsSync(dir)) {
    return false;
  }
  const entries = fs.readdirSync(dir);
  return entries.length > 0;
}

export async function initializeBotProject(context: vscode.ExtensionContext): Promise<void> {
  const picked = await vscode.window.showOpenDialog({
    canSelectFiles: false,
    canSelectFolders: true,
    canSelectMany: false,
    openLabel: 'Select Destination Folder'
  });

  if (!picked || picked.length === 0) {
    return;
  }

  const destRoot = picked[0].fsPath;
  if (hasFiles(destRoot)) {
    const confirm = await vscode.window.showWarningMessage(
      `Destination '${destRoot}' is not empty. Continue and overwrite matching files?`,
      { modal: true },
      'Overwrite'
    );
    if (confirm !== 'Overwrite') {
      return;
    }
  }

  const templateRoot = path.join(context.extensionPath, 'templates', 'bot-starter');
  if (!fs.existsSync(templateRoot)) {
    throw new Error(`missing template directory: ${templateRoot}`);
  }

  copyRecursive(templateRoot, destRoot);

  const openChoice = await vscode.window.showInformationMessage(
    `Bot project initialized at ${destRoot}`,
    'Open Folder'
  );

  if (openChoice === 'Open Folder') {
    await vscode.commands.executeCommand('vscode.openFolder', vscode.Uri.file(destRoot), false);
  }
}

export async function openBotProject(): Promise<void> {
  const picked = await vscode.window.showOpenDialog({
    canSelectFiles: false,
    canSelectFolders: true,
    canSelectMany: false,
    openLabel: 'Open Bot Folder'
  });

  if (!picked || picked.length === 0) {
    return;
  }

  await vscode.commands.executeCommand('vscode.openFolder', picked[0], false);
}
