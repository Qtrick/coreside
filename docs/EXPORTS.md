# Exports

Users can export personal tools from the tool canvas **Export** action or by asking the agent to prepare an export. Files are written only after the user chooses a destination.

## Formats

| Format | Extension | Notes |
| --- | --- | --- |
| Coreside Tool Package | `.coreside-tool.json` | Definition + safe state |
| Standalone HTML | `.html` | Trusted clock runtime updates offline |
| ZIP web bundle | `.zip.txt` (bundle) | HTML + README + manifest text package |
| PNG snapshot | `.png` | Captured in the app UI |

Incompatible formats are disabled with an explanation.

## Security

Exports strip API keys, authorization headers, protected Base Settings, and absolute local secrets. Automated tests assert common secret patterns are absent from HTML packages.

## Digital clock

The trusted `clock` component supports 12/24-hour mode, seconds, and date. Standalone HTML continues updating without Coreside or a provider.
