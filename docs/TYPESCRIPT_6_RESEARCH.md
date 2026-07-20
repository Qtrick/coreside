# TypeScript 6 Research & Migration Result

**Product:** Coreside  
**Assessed:** 2026-07-19

## Sources

- https://devblogs.microsoft.com/typescript/announcing-typescript-6-0/
- https://www.typescriptlang.org/tsconfig/
- Installed toolchain: Node v24.17.0, npm 11.13.0

## Version selection

| Item | Value |
| --- | --- |
| Previous declared | `~5.8.3` |
| Selected stable 6.x | **`6.0.3`** (`typescript: ~6.0.3`) |
| TypeScript 7 | **Not upgraded** (explicit non-goal) |

## Compatibility

| Package | Status |
| --- | --- |
| `typescript-eslint` 8.x | Peer allows `>=4.8.4 <6.1.0` — OK for 6.0.x |
| Vite 6 / Vitest 3 | No TS peer pin blocking 6.0 |
| Tauri / React 19 | Unaffected (Vite emits) |

## Config audit (`tsconfig.json`)

Already modern; no permanent `ignoreDeprecations`:

- `strict: true`
- `moduleResolution: "bundler"`
- `types: ["vitest/globals", "node"]` (explicit — TS 6 default is `[]`)
- `noUncheckedSideEffectImports: true`
- No deprecated `baseUrl`, `moduleResolution: node10`, `target: es5`

## Edge Functions

Deno Edge Runtime typechecks separately from npm `tsc`. Desktop TS 6 does not change Deno emit.

## Performance

Baseline (TS 5.8.3 typecheck): ~1.95s — see `reports/typescript-5-baseline.json`.  
Post-upgrade measurement recorded in `reports/typescript-6-result.json` after `npm run typecheck`.
