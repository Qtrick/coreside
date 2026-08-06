-- RC3.11: migrate Dock icon preference to versioned Follow macOS / manual config.

-- Legacy auto → Follow macOS (clears temporary AppKit override at runtime).
UPDATE settings
SET value = '{"schemaVersion":1,"authority":"follow_macos"}'
WHERE key IN ('dockIcon', 'dock_icon')
  AND lower(trim(value)) IN ('auto', 'follow_macos', 'system');

-- Legacy dark/light → manual Classic tiles.
UPDATE settings
SET value = '{"schemaVersion":1,"authority":"manual","artwork":"classic","style":"dark"}'
WHERE key IN ('dockIcon', 'dock_icon')
  AND lower(trim(value)) = 'dark';

UPDATE settings
SET value = '{"schemaVersion":1,"authority":"manual","artwork":"classic","style":"light"}'
WHERE key IN ('dockIcon', 'dock_icon')
  AND lower(trim(value)) = 'light';

-- Any remaining non-JSON / unrecognized values fail closed to Follow macOS.
UPDATE settings
SET value = '{"schemaVersion":1,"authority":"follow_macos"}'
WHERE key IN ('dockIcon', 'dock_icon')
  AND value NOT LIKE '{%';
