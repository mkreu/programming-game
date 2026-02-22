export type Capabilities = {
  auth_required: boolean;
  mode: string;
  registration_enabled?: boolean;
};

export type UserInfo = {
  id: number;
  username: string;
};

export type LoginResponse = {
  token: string;
  user: UserInfo;
};

export type ArtifactSummary = {
  id: number;
  owner_user_id: number;
  owner_username: string;
  name: string;
  note: string | null;
  target: string;
  is_public: boolean;
  owned_by_me: boolean;
  created_at: string;
};

export type UploadArtifactRequest = {
  name: string;
  note: string | null;
  target: string;
  elf_base64: string;
};

export type UploadArtifactResponse = {
  artifact_id: number;
};
