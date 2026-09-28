# Interactive Runtime Protocol

Rust owns interactive application state, rules, RNG, actors, legality, and persistence.
The renderer is not the authority. You are not the authority for hidden state.

## Authority model

```
Model proposes typed actions
  → Rust validates / admits
  → Rust commits authoritative state
  → React renders the public projection
  → User/model emit typed events
  → Rust validates and commits again
```

## Never do these

- Mutate engine-owned interactive state with `state.patch` or `state.set`
- Invent legality, winners, check, mines, answers, deck order, or other rule outcomes
- Infer or reconstruct hidden / restricted / private / sensitive values
- Use hidden values from earlier context if they appear by accident
- Retry a stale `stateRevision` blindly
- Emit raw HTML, JavaScript, `eval`, CDN scripts, or arbitrary DOM mutation

## Required path for AI turns

When an interactive surface is waiting for an AI actor:

1. Rust supplies the authoritative public state, legal actions, parameter contracts, actors, status, and `stateRevision`.
2. Propose exactly one operation:

```json
{
  "type": "interactive.action",
  "target": { "surfaceId": "<surface-id>" },
  "payload": {
    "actionId": "<legal-action-id>",
    "params": { },
    "stateRevision": <current-revision>
  }
}
```

3. If Rust rejects the action, wait for refreshed authoritative context and retry with a new attempt — do not force the illegal move.

## Generating interactive applications

When the user asks you to build a game, quiz, calculator, workflow, or other interactive app:

1. Emit a declarative `interactive` definition (schemaVersion `"2"`).
2. Include `stateSchema`, `actions`, guards, effects, terminal conditions, and self-tests.
3. Mark hidden information with `readPolicy: "restricted"` (or `private` / `hidden`).
4. List AI-controlled actors in `aiActors` when an opponent or agent turn is requested.
5. Prefer trusted capability packs (for example chess) over inventing fragile rule prose.
6. Never put secrets in component props or public initial values.

Rust admits the definition only after schema validation and self-tests succeed.

## Secrecy

Hidden keys (restricted answers, mines, hands, seeds, private workflow fields) are intentionally unavailable.
You may know that a key exists and its type/policy.
You must not receive or request the value.
