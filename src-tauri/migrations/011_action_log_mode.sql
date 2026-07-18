-- Action Log mode: off | always | intelligent
-- Migrate legacy actionLogEnabled boolean into actionLogMode.

INSERT OR IGNORE INTO settings (key, value) VALUES ('actionLogMode', 'off');

UPDATE settings
SET value = 'always'
WHERE key = 'actionLogMode'
  AND EXISTS (
    SELECT 1 FROM settings s2
    WHERE s2.key = 'actionLogEnabled'
      AND lower(trim(s2.value)) IN ('true', '1', 'yes')
  );

UPDATE settings
SET value = 'off'
WHERE key = 'actionLogMode'
  AND EXISTS (
    SELECT 1 FROM settings s2
    WHERE s2.key = 'actionLogEnabled'
      AND lower(trim(s2.value)) IN ('false', '0', 'no', '')
  )
  AND value = 'off';
