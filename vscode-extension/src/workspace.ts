import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';

import { configuredBotWorkspacePath, defaultArtifactTarget } from './config';

export type LocalBinary = {
  name: string;
  rootPath: string;
};

export function getWorkspaceRoot(): string | undefined {
  const configured = configuredBotWorkspacePath();
  if (configured) {
    return path.resolve(configured);
  }

  const folder = vscode.workspace.workspaceFolders?.[0];
  return folder?.uri.fsPath;
}

export function cargoTomlPath(rootPath: string): string {
  return path.join(rootPath, 'Cargo.toml');
}

export function hasCargoToml(rootPath: string): boolean {
  return fs.existsSync(cargoTomlPath(rootPath));
}

function parseExplicitBins(cargoToml: string): string[] {
  const bins: string[] = [];
  const sections = cargoToml.split('[[bin]]');
  for (let i = 1; i < sections.length; i += 1) {
    const section = sections[i];
    const match = section.match(/\bname\s*=\s*"([^"]+)"/);
    if (match?.[1]) {
      bins.push(match[1]);
    }
  }
  return bins;
}

function parseImplicitBinFiles(rootPath: string): string[] {
  const srcBin = path.join(rootPath, 'src', 'bin');
  if (!fs.existsSync(srcBin)) {
    return [];
  }

  return fs.readdirSync(srcBin)
    .filter((file) => file.endsWith('.rs'))
    .map((file) => path.basename(file, '.rs'));
}

export function listLocalBinaries(rootPath: string): LocalBinary[] {
  if (!hasCargoToml(rootPath)) {
    return [];
  }

  const cargoToml = fs.readFileSync(cargoTomlPath(rootPath), 'utf8');
  const explicit = parseExplicitBins(cargoToml);
  const implicit = parseImplicitBinFiles(rootPath);
  const merged = new Set<string>([...explicit, ...implicit]);

  return [...merged]
    .sort((a, b) => a.localeCompare(b))
    .map((name) => ({ name, rootPath }));
}

export function artifactOutputPath(rootPath: string, binName: string, targetTriple?: string): string {
  const target = targetTriple ?? defaultArtifactTarget();
  return path.join(rootPath, 'target', target, 'release', binName);
}
