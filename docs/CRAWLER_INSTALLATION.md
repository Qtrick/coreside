# Crawler installation

Web Research requires a local Crawl4AI sidecar. Without it, `engineReady` is `false` and research commands return `needs_setup`.

## Developer setup

From the repository root:

```bash
npm run crawl4ai:setup    # create .venv, install pin, Playwright Chromium via crawl4ai-setup
npm run crawl4ai:doctor   # import / environment checks
npm run crawl4ai:smoke    # short live smoke (optional)
npm run crawl4ai:test     # package tests
```

Artifacts:

| Path | Role |
| --- | --- |
| `services/crawl4ai/.venv` | Project-local Python interpreter |
| `services/crawl4ai/` | Sidecar package root |

Preferred Python: **3.11–3.13**. Avoid untested bleeding-edge versions until Crawl4AI confirms support.

## Runtime detection

Rust `get_crawler_installation` / `detect_installation`:

1. Resolve service root (`CORESIDE_CRAWLER_SERVICE_ROOT` override, else `services/crawl4ai` relative to the repo).
2. Resolve venv Python (`.venv/bin/python` or Windows `Scripts/python.exe`).
3. If missing → state `needsSetup`.
4. Probe `import crawl4ai` → Ready on success, otherwise `needsSetup` with reason.

`get_search_connection` mirrors that probe into `engineReady` / `engineReason`.

## In-app install

There is **no** Tauri command that downloads or installs the engine yet. Settings shows setup guidance and points developers at `npm run crawl4ai:setup`. Packaged consumer installers may later bundle or run an equivalent step.

## Overrides (advanced)

| Env | Purpose |
| --- | --- |
| `CORESIDE_CRAWLER_SERVICE_ROOT` | Sidecar package directory |
| `CORESIDE_CRAWLER_DATA_ROOT` | Coreside crawler data / cleanup root |
| `CRAWL4_AI_BASE_DIRECTORY` | Crawl4AI cache parent (set by sidecar helpers) |

## Cleanup

```bash
npm run crawl4ai:clean
```

Removes disposable crawler artifacts per script policy — not the Media Library.
