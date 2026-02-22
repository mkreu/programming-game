import { spawn } from 'child_process';
import * as vscode from 'vscode';

import { defaultArtifactTarget } from './config';

export async function buildBinary(rootPath: string, binName: string, targetTriple?: string): Promise<void> {
  const target = targetTriple ?? defaultArtifactTarget();

  await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: `Building ${binName}`,
      cancellable: false
    },
    async () => await runCargoBuild(rootPath, binName, target)
  );
}

function runCargoBuild(rootPath: string, binName: string, target: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const args = ['build', '--release', '--target', target, '--bin', binName];
    const child = spawn('cargo', args, {
      cwd: rootPath,
      shell: false
    });

    let stderr = '';
    let stdout = '';

    child.stdout?.on('data', (chunk: Buffer) => {
      stdout += chunk.toString();
    });

    child.stderr?.on('data', (chunk: Buffer) => {
      stderr += chunk.toString();
    });

    child.on('error', (error) => {
      reject(error);
    });

    child.on('close', (code) => {
      if (code === 0) {
        resolve();
        return;
      }

      const combined = `${stdout}\n${stderr}`.trim();
      const tail = combined.length > 3000 ? combined.slice(-3000) : combined;
      reject(new Error(`cargo build failed for '${binName}' (${target})\n${tail}`));
    });
  });
}
