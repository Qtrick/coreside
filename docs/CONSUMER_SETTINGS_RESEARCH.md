# Consumer Settings Research

**Product:** Coreside  
**Access date:** 2026-08-01  
**Public beta:** **NOT READY**  
**Priority for this phase:** Settings IA (wallpaper work **out of scope**)

## Current state (inspected)

`SettingsPanel.tsx` ships **category navigation + search** (General, Appearance, AI Access, Agent, Search & Research, Privacy & Security, Data & Storage, Accessibility, Advanced, About, Added Settings). Adaptive window sizing lives under General. Wallpapers remain under **Added → Templates** (redesign out of scope).

Privacy/Data remain **partial** (clear controls + Recovery deep links; consumer backup/restore not complete).

## Research takeaways (consumer desktop settings)

| Pattern | Apply to Coreside |
| --- | --- |
| Few primary categories with everyday labels | Prefer General / Appearance / AI Access / Agent / Search / Privacy / Data / Accessibility / Advanced / About |
| Power tools under Advanced | Recovery, Runtime Permissions, diagnostics — Recovery must stay **findable** |
| Separate “app settings” from “content templates” | Keep Wallpapers under **Added Settings → Templates** (not Base Settings structure) |
| Avoid protocol jargon in primary labels | No “orchestrator”, “gateway”, “keyring IPC” in first-run copy |
| One job per section | Don’t dump every toggle into Appearance |

Comparable consumer products (macOS System Settings, ChatGPT desktop settings, Claude desktop) group **account/connection**, **appearance**, and **privacy/data** separately from developer toggles. Coreside’s BYOK model maps to an **AI Access** category rather than an account cloud pane.

## Gap vs proposed IA

Normative proposal: [SETTINGS_INFORMATION_ARCHITECTURE.md](./SETTINGS_INFORMATION_ARCHITECTURE.md).

| Proposed | Today |
| --- | --- |
| Category sidebar / grouped nav | Missing — single scroll |
| General (startup, adaptive window) | Adaptive window lives under Appearance |
| AI Access | Present as `AiProviderSettings` without category chrome |
| Agent | Present (`AgentBehaviorSettings`) |
| Search / Privacy | Thin or absent as labeled categories |
| Advanced | Recovery + Runtime Permissions inline, not nested |
| About | Present |

## Phase recommendation

1. **Ship Settings IA navigation first** — labels + grouping only; preserve persistence keys and protect Base Settings allowlists.
2. Move Adaptive Window Sizing copy to General; keep Wallpapers in Added → Templates.
3. Soften primary language per IA language guide; leave technical detail in Advanced descriptions.
4. Do **not** block this phase on wallpaper compositing proof.

## Non-claims

- Not a full competitive teardown.
- Not WCAG certification of Settings.
- Public beta remains blocked until IA ships and key journeys (BYOK, Recovery findability) are manually signed off.
