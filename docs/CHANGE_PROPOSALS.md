# Change Proposals

**Product:** Coreside  
**Code:** `application_kernel::classify_risk` + `apply_change`  
**UI:** `ChangeProposalCard` (chat stream) + `KernelProposalPreview` (composer sticky)

Risk is classified in trusted Rust only — never by the agent. Classification takes the **highest** risk in the batch (order-independent).

## Risk levels

| Risk | When | Behavior |
| --- | --- | --- |
| `automatic` | Default (e.g. `state.set`) | Applied without proposal if policy allows |
| `lightweight` | `surface.*`, `component.*`, `setting.*`, `manifest.*`, `tool.*` | Trusted UI must set `approvalGranted`, **or** agent chat turn implies approval for non-strong generative edits |
| `strong` | Ops containing `delete`, `data.migrate`, `data.model*`, `export.prepare`, `permission.*`, `package.*` | Never auto-applied from `sourceType: "agent"`; requires trusted UI `approvalGranted` (or returns `proposalId`) |

If risk is `lightweight` or `strong`, or policy is `require_user_approval`, or `requireApproval` is set, and approval is not granted → `ChangeResult` with `proposalId`, `operations`, `summary`, and **no** apply.

`sourceType: "agent"` cannot self-approve strong changes by setting `approvalGranted: true`.

Impact text comes from `impact::summarize_operations` (consumer-facing counts + migration warnings).

## Chat wiring

1. Agent turn stores `metadata.runtimeV2` with `proposalId`, `risk`, `impactSummary`, `summary`, `operations`, `status: "pending"`.
2. `MessageBubble` renders `ChangeProposalCard` for that metadata.
3. Composer also shows a sticky `KernelProposalPreview` for the active pending proposal (hydrated on chat load / send).
4. **Apply** calls `kernel_apply_change` with `sourceType: "user"` + `approvalGranted: true`, then stamps `status: "applied"` via `set_kernel_proposal_status`.
5. **Discard** stamps `status: "discarded"` (same command / `discard_kernel_proposal`).
