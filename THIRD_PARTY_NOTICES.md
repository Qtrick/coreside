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

