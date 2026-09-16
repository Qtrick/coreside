-- RC3.12: repair Dock icon preference migration compatibility and dual-alias handling.
-- Preserves legacy 'split' as manual Split configuration.
-- Preserves valid version-1 manual choices (classic dark/light, split).
-- Preserves future schema versions (schemaVersion > 1).
-- Resolves dual aliases: promotes valid dock_icon if dockIcon is malformed, then removes dock_icon.
-- Canonicalizes invalid/malformed version-1 or corrupted JSON to Follow macOS.
-- Idempotent: re-running produces identical state.

-- 1. If an interim 024_reset_dock_icon_follow_macos was recorded in development databases, remove it.
DELETE FROM _migrations WHERE name = '024_reset_dock_icon_follow_macos';

-- 2. Normalize legacy raw string values on both alias keys
UPDATE settings
SET value = '{"schemaVersion":1,"authority":"follow_macos"}'
WHERE key IN ('dockIcon', 'dock_icon')
  AND lower(trim(value)) IN ('auto', 'follow_macos', 'system');

UPDATE settings
SET value = '{"schemaVersion":1,"authority":"manual","artwork":"classic","style":"dark"}'
WHERE key IN ('dockIcon', 'dock_icon')
  AND lower(trim(value)) = 'dark';

UPDATE settings
SET value = '{"schemaVersion":1,"authority":"manual","artwork":"classic","style":"light"}'
WHERE key IN ('dockIcon', 'dock_icon')
  AND lower(trim(value)) = 'light';

UPDATE settings
SET value = '{"schemaVersion":1,"authority":"manual","artwork":"split","style":"original"}'
WHERE key IN ('dockIcon', 'dock_icon')
  AND lower(trim(value)) = 'split';

-- 3. Resolve dual-alias conflict: if dockIcon is invalid or malformed, but dock_icon is valid, adopt dock_icon into dockIcon.
UPDATE settings
SET value = (SELECT s2.value FROM settings s2 WHERE s2.key = 'dock_icon')
WHERE key = 'dockIcon'
  AND EXISTS (
    SELECT 1 FROM settings s2
    WHERE s2.key = 'dock_icon'
      AND json_valid(s2.value) = 1
      AND (
        json_extract(s2.value, '$.schemaVersion') > 1
        OR (
          json_extract(s2.value, '$.schemaVersion') = 1
          AND (
            json_extract(s2.value, '$.authority') = 'follow_macos'
            OR (
              json_extract(s2.value, '$.authority') = 'manual'
              AND (
                (json_extract(s2.value, '$.artwork') = 'classic' AND json_extract(s2.value, '$.style') IN ('dark', 'light'))
                OR (json_extract(s2.value, '$.artwork') = 'split')
              )
            )
          )
        )
      )
  )
  AND (
    json_valid(value) = 0
    OR json_extract(value, '$.schemaVersion') IS NULL
    OR (
      json_extract(value, '$.schemaVersion') = 1
      AND json_extract(value, '$.authority') NOT IN ('follow_macos', 'manual')
    )
    OR (
      json_extract(value, '$.schemaVersion') = 1
      AND json_extract(value, '$.authority') = 'manual'
      AND (
        json_extract(value, '$.artwork') NOT IN ('classic', 'split')
        OR (json_extract(value, '$.artwork') = 'classic' AND json_extract(value, '$.style') NOT IN ('dark', 'light'))
      )
    )
  );

-- 4. If dockIcon did not exist at all, but dock_icon exists, promote it to dockIcon
INSERT OR IGNORE INTO settings (key, value)
SELECT 'dockIcon', value FROM settings WHERE key = 'dock_icon';

-- 5. Delete dock_icon row so dockIcon is the sole canonical key
DELETE FROM settings WHERE key = 'dock_icon';

-- 6. Canonicalize any remaining invalid / malformed version 1 or non-JSON values in dockIcon:
--    (Future schema versions schemaVersion > 1 are preserved untouched!)
UPDATE settings
SET value = '{"schemaVersion":1,"authority":"follow_macos"}'
WHERE key = 'dockIcon'
  AND (
    json_valid(value) = 0
    OR json_extract(value, '$.schemaVersion') IS NULL
    OR (
      json_extract(value, '$.schemaVersion') = 1
      AND (
        json_extract(value, '$.authority') NOT IN ('follow_macos', 'manual')
        OR (
          json_extract(value, '$.authority') = 'manual'
          AND (
            json_extract(value, '$.artwork') NOT IN ('classic', 'split')
            OR (json_extract(value, '$.artwork') = 'classic' AND json_extract(value, '$.style') NOT IN ('dark', 'light'))
          )
        )
      )
    )
  );
