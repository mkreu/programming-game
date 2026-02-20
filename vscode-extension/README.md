# Programming Game RaceHub VSCode Extension

Minimal connector extension for uploading ELF artifacts to RaceHub.

## Commands

- `RaceHub: Configure Server URL`
- `RaceHub: Login`
- `RaceHub: Upload Artifact File`

## Behavior

- If server reports `auth_required=true`, upload requires login and uses bearer token from VSCode secret storage.
- If server reports `auth_required=false` (standalone mode), upload works without login.
