# Media Library

Local media assets under app data `Coreside/media/`, indexed in `media_assets` (migrations `007`–`008`).

## Import flow

1. Frontend requests import (user approval in UI).
2. `import_media_asset_cmd` downloads via SSRF-safe HTTP.
3. Magic-byte + MIME allowlist validation rejects executables and HTML.
4. Metadata enrichment parses image dimensions (PNG/JPEG/GIF/WebP) and MP4/QuickTime duration when possible.
5. Oversized edges (`MAX_IMAGE_EDGE_PX`) or video duration (`MAX_VIDEO_DURATION_MS`) are rejected.
6. A JPEG thumbnail (max edge 320px) is generated for still images and stored as `{id}.thumb.jpg`.
7. Dedup by SHA-256 `content_hash` (returns existing row if duplicate).

## Limits

| Kind | Max size |
| --- | --- |
| Image | 25 MiB |
| Animated / GIF | 15 MiB |
| Video | 200 MiB |
| Image edge | 16 384 px |
| Thumbnail edge | 320 px |

Allowed MIME types: JPEG, PNG, WebP, GIF, AVIF; MP4, WebM, QuickTime.

## Commands

| Command | Description |
| --- | --- |
| `list_media_assets_cmd` | List by optional `projectId` |
| `get_media_asset_cmd` | Single asset metadata |
| `get_media_asset_src_cmd` | Full local file path (validated) |
| `get_media_asset_thumb_src_cmd` | Thumbnail path when available |
| `delete_media_asset_cmd` | Remove DB row + original + thumbnail |
| `import_media_asset_cmd` | Validated download + persist |
| `touch_media_asset_cmd` | Update `lastUsedAt` |

## Project isolation

`project_id` is optional FK; listing can scope to a project. Assets are not auto-linked to wallpapers until referenced by `assetId`.
