import { resolveServerUrl } from './config';
import {
  ArtifactSummary,
  Capabilities,
  LoginResponse,
  UploadArtifactRequest,
  UploadArtifactResponse
} from './types';

function authHeaders(token?: string): Record<string, string> {
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }
  return headers;
}

export async function fetchCapabilities(): Promise<Capabilities> {
  const resp = await fetch(`${resolveServerUrl()}/api/v1/capabilities`);
  if (!resp.ok) {
    throw new Error(`capabilities failed: ${resp.status} ${resp.statusText}`);
  }
  return await resp.json() as Capabilities;
}

export async function loginRequest(username: string, password: string): Promise<LoginResponse> {
  const resp = await fetch(`${resolveServerUrl()}/api/v1/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password })
  });

  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(`login failed: ${resp.status} ${text}`);
  }

  return await resp.json() as LoginResponse;
}

export async function listArtifacts(token?: string): Promise<ArtifactSummary[]> {
  const resp = await fetch(`${resolveServerUrl()}/api/v1/artifacts`, {
    headers: authHeaders(token)
  });

  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(`list artifacts failed: ${resp.status} ${text}`);
  }

  return await resp.json() as ArtifactSummary[];
}

export async function uploadArtifact(request: UploadArtifactRequest, token?: string): Promise<UploadArtifactResponse> {
  const resp = await fetch(`${resolveServerUrl()}/api/v1/artifacts`, {
    method: 'POST',
    headers: authHeaders(token),
    body: JSON.stringify(request)
  });

  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(`artifact upload failed: ${resp.status} ${text}`);
  }

  return await resp.json() as UploadArtifactResponse;
}

export async function deleteArtifact(id: number, token?: string): Promise<void> {
  const resp = await fetch(`${resolveServerUrl()}/api/v1/artifacts/${id}`, {
    method: 'DELETE',
    headers: authHeaders(token)
  });

  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(`delete artifact failed: ${resp.status} ${text}`);
  }
}

export async function updateArtifactVisibility(id: number, isPublic: boolean, token?: string): Promise<void> {
  const resp = await fetch(`${resolveServerUrl()}/api/v1/artifacts/${id}/visibility`, {
    method: 'PATCH',
    headers: authHeaders(token),
    body: JSON.stringify({ is_public: isPublic })
  });

  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(`update visibility failed: ${resp.status} ${text}`);
  }
}
