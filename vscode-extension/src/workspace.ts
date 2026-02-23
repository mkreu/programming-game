import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';

import { configuredBotWorkspacePath, defaultArtifactTarget } from './config';

export type LocalBinary = {
  name: string;
  rootPath: string;
  sourcePath?: string;
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

function normalizePath(filePath: string): string {
  return path.normalize(filePath);
}

function parseExplicitBins(cargoToml: string, rootPath: string): LocalBinary[] {
  const bins: LocalBinary[] = [];
  const sections = cargoToml.split('[[bin]]');
  for (let i = 1; i < sections.length; i += 1) {
    const section = sections[i];
    const nameMatch = section.match(/\bname\s*=\s*"([^"]+)"/);
    if (!nameMatch?.[1]) {
      continue;
    }

    const name = nameMatch[1];
    const pathMatch = section.match(/\bpath\s*=\s*"([^"]+)"/);

    if (pathMatch?.[1]) {
      bins.push({
        name,
        rootPath,
        sourcePath: normalizePath(path.resolve(rootPath, pathMatch[1]))
      });
      continue;
    }

    const inferredPath = path.join(rootPath, 'src', 'bin', `${name}.rs`);
    bins.push({
      name,
      rootPath,
      sourcePath: fs.existsSync(inferredPath) ? normalizePath(inferredPath) : undefined
    });
  }
  return bins;
}

function parseImplicitBinFiles(rootPath: string): LocalBinary[] {
  const srcBin = path.join(rootPath, 'src', 'bin');
  if (!fs.existsSync(srcBin)) {
    return [];
  }

  return fs.readdirSync(srcBin)
    .filter((file) => file.endsWith('.rs'))
    .map((file) => {
      const filePath = path.join(srcBin, file);
      return {
        name: path.basename(file, '.rs'),
        rootPath,
        sourcePath: normalizePath(filePath)
      };
    });
}

export function listLocalBinaries(rootPath: string): LocalBinary[] {
  if (!hasCargoToml(rootPath)) {
    return [];
  }

  const cargoToml = fs.readFileSync(cargoTomlPath(rootPath), 'utf8');
  const explicit = parseExplicitBins(cargoToml, rootPath);
  const implicit = parseImplicitBinFiles(rootPath);
  const byName = new Map<string, LocalBinary>();

  for (const bin of explicit) {
    byName.set(bin.name, bin);
  }

  for (const bin of implicit) {
    const existing = byName.get(bin.name);
    if (!existing) {
      byName.set(bin.name, bin);
      continue;
    }

    if (!existing.sourcePath && bin.sourcePath) {
      byName.set(bin.name, bin);
    }
  }

  return [...byName.values()].sort((a, b) => a.name.localeCompare(b.name));
}

export function findLocalBinaryForFile(rootPath: string, filePath: string): LocalBinary | undefined {
  const target = normalizePath(filePath);
  return listLocalBinaries(rootPath).find((bin) => bin.sourcePath && normalizePath(bin.sourcePath) === target);
}

export function artifactOutputPath(rootPath: string, binName: string, targetTriple?: string): string {
  const target = targetTriple ?? defaultArtifactTarget();
  return path.join(rootPath, 'target', target, 'release', binName);
}
