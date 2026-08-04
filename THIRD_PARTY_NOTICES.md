# Third-party notices

Coreside includes third-party software. This file records attributions required for redistribution.

## Crawl4AI

- **Package:** [Crawl4AI](https://github.com/unclecode/crawl4ai)
- **Version used by Coreside:** 0.9.2 (pinned in `services/crawl4ai`)
- **License:** Apache License 2.0
- **Copyright:** Crawl4AI authors and contributors
- **SPDX:** `Apache-2.0`

Crawl4AI is used as a local Web Research sidecar. The Coreside product UI labels the feature **Web Research**; Crawl4AI is internal engine attribution.

Full Apache-2.0 license text: https://www.apache.org/licenses/LICENSE-2.0

```
Copyright notice from the Crawl4AI distribution (Apache-2.0).

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```

Crawl4AI itself depends on additional open-source packages (including Playwright / Chromium components). Those licenses apply to their respective distributions under `services/crawl4ai/.venv` when installed.

## Exa

- **Service:** [Exa](https://exa.ai) Search API
- **Use in Coreside:** Optional indexed web-discovery provider (BYOK / `EXA_API_KEY`)
- **Docs:** https://docs.exa.ai

Exa is a third-party network service, not redistributed source code. API usage is subject to Exa’s terms and pricing. Coreside does not ship Exa credentials.

## Partial Update (direct port + secure adaptation)

- **Project:** [Partial Update](https://github.com/philholden/partialupdate)
- **License:** MIT
- **Copyright:** Copyright (c) 2026 Phil Holden
- **Archive SHA-256:** `8666c226cb875deae8a73e6d2c7c09965f311b09c3db15ea1d1305261a3eb607`
- **Use in Coreside:** Valuable algorithms and interaction mechanics are **directly translated or substantially adapted** into Coreside’s trusted Tauri + Rust + React stack (incremental stream parse, queue ownership, fork/snapshot indexing, structured form continuation, filtered subscriber delivery). Unsafe HTML/JS/CSS execution, CDN injection, and hidden iframe forms are **rejected**.
- **Policy / provenance:** `docs/PARTIAL_UPDATE_DIRECT_PORT_POLICY.md`, `docs/PARTIAL_UPDATE_PORT_PROVENANCE.md`, `reports/partial-update-port-map.json`
- **Reference extract (not committed):** `.reference/partial-update/`

When Coreside source contains a substantial translation of Partial Update logic, the destination file carries an attribution comment naming the original path/symbol.

```
MIT License

Copyright (c) 2026 Phil Holden

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Vendo (conceptual inspiration)

- **Project:** [Vendo](https://github.com/vendo-ai/vendo)
- **License:** Apache License 2.0
- **Copyright:** Copyright 2026 Vendo
- **NOTICE:** Vendo distributes a NOTICE file identifying Vendo and an OpenUI MIT subcomponent used in Vendo’s sandbox bundle. Coreside does **not** redistribute that sandbox bundle.
- **Use in Coreside:** Architectural inspiration for the generated-application consent model — descriptor hashing, remembered grants, single-use approvals, presence-aware policy, and a single execution choke point. **No Vendo TypeScript packages were copied** into the Coreside application tree; the runtime is a Coreside-native Rust + React implementation.
- **Reference extract (not a production dependency):** `.reference/vendo/`
- **Docs:** `docs/VENDO_REFERENCE_AND_ADOPTION_AUDIT.md`

```
Vendo
Copyright 2026 Vendo

This product includes software developed at Vendo (https://vendo.run).

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreement to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```

