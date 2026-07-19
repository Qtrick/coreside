# Partial Update Video Analysis

- **Title:** What if AI replies in HTML not Markdown?
- **URL:** https://www.youtube.com/watch?v=f39MnczcJZA
- **Playlist:** https://www.youtube.com/watch?v=f39MnczcJZA&list=PLdYOUh501F8o
- **Access date:** 2026-07-18
- **Transcript source:** YouTube ASR via `youtube-transcript-api` (331 cue lines)
- **Official subtitles:** none; auto-generated English used
- **Duration (approx):** ~14.5 minutes (last cue ~00:14:xx)

## Timestamped demonstrations

| Timestamp | Demonstration | Source mapping | Coreside status | Adoption |
|---|---|---|---|---|
| 00:00:23–00:00:31 | Out-of-order streamed HTML into placeholders (Chrome DPU) | `UpdateStreamParser`, markers | Different: NDJSON op frames | Adapt streaming, not HTML |
| 00:00:34–00:00:52 | HTML/SVG/CSS/JS/forms; tic-tac-toe | `initialPrompt.md` forms | Trusted components + forms pack | Safe adaptation |
| 00:02:04–00:02:50 | Translate FR/EN + silent spelling + CSS language toggle | SERVER include/exclude + silent | Deferred translation; silent ops now | Defer i18n |
| 00:03:14–00:03:38 | Multiplayer Connect 4 | Multiuser DO + forms | Deferred collaboration | Defer |
| 00:03:47–00:05:12 | Wikipedia full-page redesign, SVG/MathML navigation | Full HTML replace | Reject full chrome replace; SVG/Math packs | Reject / adapt packs |
| 00:05:17–00:05:22 | Whimsical canvas typing effect | Arbitrary JS/canvas | Declarative canvas limits | Adapt carefully |
| 00:05:34–00:08:42 | CSS/Figma iteration, conic gradient animation | HTML+CSS updates | Design-token / preview workflow | Adapt via editor pack |
| 00:08:53–00:10:15 | CDN CodeMirror playground save-to-context | CDN injection | Bundled codeEditor, no exec, no CDN | Safe adaptation |
| 00:10:45–00:11:38 | Tailwind lunch menu + shared table orders | CDN + multiuser | Reject Tailwind CDN; defer multiuser | Reject / defer |
| 00:11:46–00:14:04 | Private messages / Hangman secrecy; alignment failure | include routing | Defer collaboration | Defer |

## Notes

- Voice dictation is mentioned in the Partial Update README but **was not demonstrated** in this video’s ASR transcript.
- Chrome version in narration says “Chrome 150”; Chrome blog (accessed 2026-07-18) documents experimental DPU from Chrome 148 flags — treat as evolving platform status.
