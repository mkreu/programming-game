import * as vscode from 'vscode';

import { openLoginWebview } from './views/loginWebview';

const TOKEN_KEY = 'racehub.token';

export async function readToken(context: vscode.ExtensionContext): Promise<string | undefined> {
  return await context.secrets.get(TOKEN_KEY) ?? undefined;
}

export async function clearToken(context: vscode.ExtensionContext): Promise<void> {
  await context.secrets.delete(TOKEN_KEY);
}

export async function loginViaWebview(context: vscode.ExtensionContext): Promise<boolean> {
  return await openLoginWebview(context);
}
