# P0.6 Linkup official-source notes

Reviewed 2026-08-10. These notes define the narrow integration contract, not a
runtime dependency on Linkup reasoning.

- [Search overview](https://docs.linkup.so/pages/documentation/endpoints/search/overview):
  use `POST /v1/search`; `fast` is index retrieval and does not use an LLM to
  reinterpret the query. Coreside requests `outputType: searchResults` rather
  than an answer-producing mode.
- [Search reference](https://docs.linkup.so/pages/documentation/endpoints/search/reference):
  search accepts `q`, `depth`, `outputType`, `maxResults`, and optional
  `includeDomains`.
- [Fetch reference](https://docs.linkup.so/pages/documentation/endpoints/fetch/reference):
  fetch accepts a URL and returns extracted content. Coreside requests no raw
  HTML, images, or JavaScript rendering, and treats returned Markdown as
  untrusted evidence.
- [Authentication](https://docs.linkup.so/pages/documentation/platform/authentication):
  requests use a bearer API key. Local credentials live only in the OS keyring
  (with `LINKUP_API_KEY` as a development fallback); hosted credentials use
  the `LINKUP_API_KEY` Edge-function secret.

Out of scope by design: Linkup Research, provider-generated answer synthesis,
raw HTML/script execution, provider tool delegation, and forwarding retrieved
content into trusted UI or action instructions.
