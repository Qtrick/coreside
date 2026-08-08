-- P0.2: Manual Dock icon selection is product-dormant. Reset stored preferences
-- to Follow macOS so stale Classic/Split values cannot keep a PNG Dock override
-- after the user-facing selector is removed. Schema and manual implementation
-- remain; reactivation should start users on Follow macOS again.
-- Idempotent: re-running leaves Follow macOS rows unchanged.

UPDATE settings
SET value = '{"schemaVersion":1,"authority":"follow_macos"}'
WHERE key IN ('dockIcon', 'dock_icon')
  AND (
    value LIKE '%"authority":"manual"%'
    OR lower(trim(value)) IN ('dark', 'light', 'split')
  );
