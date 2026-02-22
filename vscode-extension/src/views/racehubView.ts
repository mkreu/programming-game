import * as fs from 'fs';
import * as vscode from 'vscode';

import {
  deleteArtifact,
  fetchCapabilities,
  listArtifacts,
  updateArtifactVisibility,
  uploadArtifact
} from '../api';
import { clearToken, readToken } from '../auth';
import { buildBinary } from '../build';
import { defaultArtifactTarget } from '../config';
import { ArtifactSummary } from '../types';
import {
  artifactOutputPath,
  getWorkspaceRoot,
  hasCargoToml,
  listLocalBinaries,
  LocalBinary
} from '../workspace';

type ViewState = 'loggedOut' | 'needsWorkspace' | 'ready';

type RootNodeKind = 'localRoot' | 'remoteRoot';

type Node =
  | { kind: RootNodeKind }
  | { kind: 'localBin'; bin: LocalBinary }
  | { kind: 'remoteArtifact'; artifact: ArtifactSummary }
  | { kind: 'message'; message: string }
  | { kind: 'action'; label: string; description?: string; command: string; arguments?: unknown[] };

export class RaceHubItem extends vscode.TreeItem {
  constructor(public readonly node: Node) {
    super(itemLabel(node), collapsibleState(node));
    this.contextValue = contextValue(node);

    if (node.kind === 'localBin') {
      this.description = node.bin.rootPath;
      this.tooltip = `${node.bin.name} (${node.bin.rootPath})`;
      this.iconPath = new vscode.ThemeIcon('symbol-method');
    }

    if (node.kind === 'remoteArtifact') {
      const artifact = node.artifact;
      this.description = `${artifact.owner_username} · ${artifact.is_public ? 'public' : 'private'}`;
      this.tooltip = `${artifact.name} (#${artifact.id})`;
      this.iconPath = new vscode.ThemeIcon('package');
    }

    if (node.kind === 'message') {
      this.iconPath = new vscode.ThemeIcon('info');
      this.tooltip = node.message;
    }

    if (node.kind === 'action') {
      this.description = node.description;
      this.iconPath = new vscode.ThemeIcon('play-circle');
      this.command = {
        command: node.command,
        title: node.label,
        arguments: node.arguments
      };
    }

    if (node.kind === 'localRoot' || node.kind === 'remoteRoot') {
      this.iconPath = new vscode.ThemeIcon('list-tree');
    }
  }
}

function itemLabel(node: Node): string {
  switch (node.kind) {
    case 'localRoot':
      return 'Local Binaries';
    case 'remoteRoot':
      return 'Remote Artifacts';
    case 'localBin':
      return node.bin.name;
    case 'remoteArtifact':
      return node.artifact.name;
    case 'message':
      return node.message;
    case 'action':
      return node.label;
  }
}

function collapsibleState(node: Node): vscode.TreeItemCollapsibleState {
  if (node.kind === 'localRoot' || node.kind === 'remoteRoot') {
    return vscode.TreeItemCollapsibleState.Expanded;
  }
  return vscode.TreeItemCollapsibleState.None;
}

function contextValue(node: Node): string | undefined {
  switch (node.kind) {
    case 'localRoot':
      return 'localRoot';
    case 'remoteRoot':
      return 'remoteRoot';
    case 'localBin':
      return 'localBin';
    case 'remoteArtifact':
      return node.artifact.owned_by_me ? 'remoteArtifactOwned' : 'remoteArtifact';
    case 'message':
      return 'message';
    case 'action':
      return 'actionItem';
  }
}

export class RaceHubViewProvider implements vscode.TreeDataProvider<RaceHubItem> {
  private readonly onDidChangeTreeDataEmitter = new vscode.EventEmitter<RaceHubItem | undefined>();
  readonly onDidChangeTreeData = this.onDidChangeTreeDataEmitter.event;

  private state: ViewState = 'loggedOut';
  private token: string | undefined;
  private workspaceRoot: string | undefined;
  private localBinaries: LocalBinary[] = [];
  private artifacts: ArtifactSummary[] = [];
  private artifactsError: string | undefined;
  private stateMessage: string | undefined;

  constructor(private readonly context: vscode.ExtensionContext) {}

  async refreshArtifacts(): Promise<void> {
    this.artifacts = [];
    this.artifactsError = undefined;
    this.stateMessage = undefined;
    this.localBinaries = [];
    this.workspaceRoot = getWorkspaceRoot();

    try {
      const caps = await fetchCapabilities();
      if (caps.auth_required) {
        this.token = await readToken(this.context);
        if (!this.token) {
          this.state = 'loggedOut';
          this.stateMessage = 'Not logged in to RaceHub.';
          await this.pushContext();
          this.onDidChangeTreeDataEmitter.fire(undefined);
          return;
        }
      } else {
        this.token = undefined;
      }

      if (!this.workspaceRoot || !hasCargoToml(this.workspaceRoot)) {
        this.state = 'needsWorkspace';
        this.stateMessage = 'Open or initialize a bot workspace to continue.';
        await this.pushContext();
        this.onDidChangeTreeDataEmitter.fire(undefined);
        return;
      }

      this.localBinaries = listLocalBinaries(this.workspaceRoot);
      if (this.localBinaries.length === 0) {
        this.state = 'needsWorkspace';
        this.stateMessage = `No binaries found in ${this.workspaceRoot}`;
        await this.pushContext();
        this.onDidChangeTreeDataEmitter.fire(undefined);
        return;
      }

      this.state = 'ready';
      this.artifacts = await listArtifacts(this.token);
      await this.pushContext();
      this.onDidChangeTreeDataEmitter.fire(undefined);
    } catch (error) {
      const message = String(error);
      if (message.includes('401')) {
        await clearToken(this.context);
        this.state = 'loggedOut';
        this.stateMessage = 'Session expired. Please log in again.';
      } else {
        this.state = 'loggedOut';
        this.stateMessage = message;
      }
      await this.pushContext();
      this.onDidChangeTreeDataEmitter.fire(undefined);
    }
  }

  private async pushContext(): Promise<void> {
    await vscode.commands.executeCommand('setContext', 'racehub.state', this.state);
  }

  getTreeItem(element: RaceHubItem): vscode.TreeItem {
    return element;
  }

  getChildren(element?: RaceHubItem): vscode.ProviderResult<RaceHubItem[]> {
    if (!element) {
      if (this.state === 'loggedOut') {
        return [
          new RaceHubItem({ kind: 'message', message: this.stateMessage ?? 'Not logged in.' }),
          new RaceHubItem({ kind: 'action', label: 'Log In', command: 'racehub.login' }),
          new RaceHubItem({ kind: 'action', label: 'Configure Server URL', command: 'racehub.configureServer' })
        ];
      }

      if (this.state === 'needsWorkspace') {
        return [
          new RaceHubItem({ kind: 'message', message: this.stateMessage ?? 'Bot workspace needed.' }),
          new RaceHubItem({ kind: 'action', label: 'Initialize Bot Project', command: 'racehub.initializeBotProject' }),
          new RaceHubItem({ kind: 'action', label: 'Open Bot Project', command: 'racehub.openBotProject' })
        ];
      }

      return [
        new RaceHubItem({ kind: 'localRoot' }),
        new RaceHubItem({ kind: 'remoteRoot' })
      ];
    }

    const node = element.node;
    if (node.kind === 'localRoot') {
      return this.localBinaries.map((bin) => new RaceHubItem({ kind: 'localBin', bin }));
    }

    if (node.kind === 'remoteRoot') {
      if (this.artifactsError) {
        return [new RaceHubItem({ kind: 'message', message: `Error: ${this.artifactsError}` })];
      }
      if (this.artifacts.length === 0) {
        return [new RaceHubItem({ kind: 'message', message: 'No artifacts found' })];
      }
      return this.artifacts.map((artifact) => new RaceHubItem({ kind: 'remoteArtifact', artifact }));
    }

    return [];
  }

  async buildBinaryItem(item?: RaceHubItem): Promise<void> {
    const node = item?.node;
    if (!node || node.kind !== 'localBin') {
      return;
    }
    await buildBinary(node.bin.rootPath, node.bin.name);
    void vscode.window.showInformationMessage(`Built binary '${node.bin.name}'`);
  }

  async revealElfPath(item?: RaceHubItem): Promise<void> {
    const node = item?.node;
    if (!node || node.kind !== 'localBin') {
      return;
    }

    const elfPath = artifactOutputPath(node.bin.rootPath, node.bin.name);
    if (!fs.existsSync(elfPath)) {
      throw new Error(`ELF not found: ${elfPath}. Build the binary first.`);
    }

    await vscode.commands.executeCommand('revealFileInOS', vscode.Uri.file(elfPath));
  }

  async buildAndUploadBinary(item?: RaceHubItem): Promise<void> {
    const node = item?.node;
    if (!node || node.kind !== 'localBin') {
      return;
    }

    await this.uploadFromLocalBinary(node.bin, node.bin.name);
    await this.refreshArtifacts();
  }

  async replaceArtifact(item?: RaceHubItem): Promise<void> {
    const node = item?.node;
    if (!node || node.kind !== 'remoteArtifact') {
      return;
    }
    const artifact = node.artifact;
    if (!artifact.owned_by_me) {
      throw new Error('You can only replace artifacts you own.');
    }

    const picked = await vscode.window.showQuickPick(
      this.localBinaries.map((bin) => ({ label: bin.name, description: bin.rootPath, bin })),
      { title: `Replace artifact '${artifact.name}' with local binary` }
    );
    if (!picked) {
      return;
    }

    await this.uploadFromLocalBinary(picked.bin, artifact.name);

    try {
      await deleteArtifact(artifact.id, this.token);
      void vscode.window.showInformationMessage(
        `Replaced artifact '${artifact.name}' by uploading new build and deleting #${artifact.id}`
      );
    } catch (error) {
      void vscode.window.showWarningMessage(
        `Uploaded replacement, but failed to delete old artifact #${artifact.id}: ${String(error)}`
      );
    }

    await this.refreshArtifacts();
  }

  async deleteArtifact(item?: RaceHubItem): Promise<void> {
    const node = item?.node;
    if (!node || node.kind !== 'remoteArtifact') {
      return;
    }

    const artifact = node.artifact;
    if (!artifact.owned_by_me) {
      throw new Error('You can only delete artifacts you own.');
    }

    const confirmed = await vscode.window.showWarningMessage(
      `Delete artifact '${artifact.name}' (#${artifact.id})?`,
      { modal: true },
      'Delete'
    );

    if (confirmed !== 'Delete') {
      return;
    }

    await deleteArtifact(artifact.id, this.token);
    void vscode.window.showInformationMessage(`Deleted artifact '${artifact.name}' (#${artifact.id})`);
    await this.refreshArtifacts();
  }

  async toggleVisibility(item?: RaceHubItem): Promise<void> {
    const node = item?.node;
    if (!node || node.kind !== 'remoteArtifact') {
      return;
    }

    const artifact = node.artifact;
    if (!artifact.owned_by_me) {
      throw new Error('You can only change visibility of artifacts you own.');
    }

    const nextPublic = !artifact.is_public;
    await updateArtifactVisibility(artifact.id, nextPublic, this.token);
    void vscode.window.showInformationMessage(
      `Artifact '${artifact.name}' is now ${nextPublic ? 'public' : 'private'}`
    );
    await this.refreshArtifacts();
  }

  private async uploadFromLocalBinary(bin: LocalBinary, defaultName: string): Promise<void> {
    await buildBinary(bin.rootPath, bin.name);

    const elfPath = artifactOutputPath(bin.rootPath, bin.name);
    if (!fs.existsSync(elfPath)) {
      throw new Error(`ELF not found after build: ${elfPath}`);
    }

    const name = await vscode.window.showInputBox({
      title: 'Artifact Name',
      value: defaultName
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
      value: defaultArtifactTarget()
    });
    if (!target) {
      return;
    }

    const bytes = fs.readFileSync(elfPath);

    const data = await uploadArtifact(
      {
        name,
        note: note && note.trim().length > 0 ? note.trim() : null,
        target,
        elf_base64: bytes.toString('base64')
      },
      this.token
    );

    void vscode.window.showInformationMessage(`Artifact uploaded: #${data.artifact_id} from '${bin.name}'`);
  }
}
