-- One-time workspace appearance baseline reset (RC polish).
-- Preserves chats, projects, tools, media, credentials, and project wallpapers.
-- Clears tinted/custom global chrome so the shell returns to system light/dark,
-- no workspace wallpaper, and solid interface transparency.

DELETE FROM settings WHERE key IN (
  'wallpaper',
  'wallpaperJson',
  'wallpaper_json',
  'interfaceTransparency',
  'interface_transparency',
  'accentPrimaryLight',
  'accentPrimaryDark',
  'accentSecondaryLight',
  'accentSecondaryDark',
  'backgroundLight',
  'backgroundDark',
  'surfaceLight',
  'surfaceDark',
  'surfaceMutedLight',
  'surfaceMutedDark',
  'borderLight',
  'borderDark',
  'textPrimaryLight',
  'textPrimaryDark',
  'textSecondaryLight',
  'textSecondaryDark',
  'accent_primary_light',
  'accent_primary_dark',
  'accent_secondary_light',
  'accent_secondary_dark',
  'background_light',
  'background_dark',
  'surface_light',
  'surface_dark',
  'surface_muted_light',
  'surface_muted_dark',
  'border_light',
  'border_dark',
  'text_primary_light',
  'text_primary_dark',
  'text_secondary_light',
  'text_secondary_dark'
);

INSERT INTO settings (key, value) VALUES ('theme', 'system')
ON CONFLICT(key) DO UPDATE SET value = excluded.value;

INSERT INTO settings (key, value) VALUES ('interfaceTransparency', '0')
ON CONFLICT(key) DO UPDATE SET value = excluded.value;
