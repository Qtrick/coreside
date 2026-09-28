//! Integration tests for authoritative interactive persistence against real SQLite.

use serde_json::{json, Map, Value};
use tempfile::TempDir;

use super::interactive::{
    ai_action_budget_allows_retry, ai_turn_context, dispatch, get_view, history,
    new_ai_action_attempt_id, probe_ai_action, probe_ai_operations, reconstruct, replay_view, undo,
    AiActionRejection, InteractiveView, Origin,
};
use super::limits::MAX_REPAIR_ATTEMPTS;
use super::operations::{AppOperation, OperationTarget};
use super::rules_engine::{InteractiveAppDefinition, RNG_STATE_KEY};
use super::rules_fixtures::get_fixture;
use super::surfaces::{
    create_inline_surface, get_surface, get_surface_state_with_revision,
    save_surface_state_user_cas, update_surface_definition,
};
use super::transactions::{apply_transaction, create_transaction, undo_transaction};
use crate::db::{create_conversation, Database, DbError, DEFAULT_WORKSPACE_ID};

struct Env {
    _dir: TempDir,
    db: Database,
    conv: String,
}

fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Database::open_path(&dir.path().join("interactive.db")).unwrap();
    let conv = create_conversation(&mut db, DEFAULT_WORKSPACE_ID, "Games", None)
        .unwrap()
        .id;
    Env {
        _dir: dir,
        db,
        conv,
    }
}

fn fixture_json(name: &str) -> Value {
    serde_json::to_value(get_fixture(name).unwrap()).unwrap()
}

fn surface_def(interactive: Value) -> Value {
    json!({
        "id": "app-doc",
        "name": "App",
        "layout": "stack",
        "components": [{ "id": "title", "type": "heading", "props": { "text": "App" } }],
        "stateContracts": [
            { "key": "board", "type": "array", "initialValue": [], "writePolicy": "user" },
            { "key": "note", "type": "string", "initialValue": "", "writePolicy": "user" }
        ],
        "interactive": interactive
    })
}

fn create(e: &mut Env, interactive: Value) -> String {
    create_inline_surface(
        &mut e.db,
        &e.conv,
        None,
        None,
        "App",
        &surface_def(interactive),
        &[],
    )
    .unwrap()
    .id
}

fn raw_state(db: &Database, sid: &str) -> (Value, i64) {
    get_surface_state_with_revision(db, sid).unwrap()
}

fn owned_of(def: &InteractiveAppDefinition, state: &Value) -> Value {
    let owned = def.owned_keys();
    let map: Map<String, Value> = state
        .as_object()
        .unwrap()
        .iter()
        .filter(|(k, _)| owned.contains(*k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Value::Object(map)
}

fn log_count(db: &Database, sid: &str) -> i64 {
    db.conn()
        .query_row(
            "SELECT COUNT(*) FROM interactive_action_log WHERE surface_id = ?1",
            [sid],
            |r| r.get(0),
        )
        .unwrap()
}

fn raw_state_json(db: &Database, sid: &str) -> (String, i64) {
    db.conn()
        .query_row(
            "SELECT state_json, state_revision FROM surface_state WHERE surface_id = ?1",
            [sid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
}

fn write_raw_state(db: &Database, sid: &str, state: &Value) {
    db.conn()
        .execute(
            "UPDATE surface_state SET state_json = ?1 WHERE surface_id = ?2",
            rusqlite::params![state.to_string(), sid],
        )
        .unwrap();
}

fn act(e: &mut Env, sid: &str, event: &str, action: &str, params: Value) -> InteractiveView {
    let rev = raw_state(&e.db, sid).1;
    dispatch(&mut e.db, sid, rev, event, action, &params, Origin::User)
        .unwrap_or_else(|err| panic!("{action} {params} failed: {err}"))
}

fn state_op(op_type: &str, sid: &str, payload: Value) -> AppOperation {
    AppOperation {
        id: format!("op-{}", uuid::Uuid::new_v4()),
        op_type: op_type.into(),
        target: OperationTarget {
            surface_id: Some(sid.into()),
            ..Default::default()
        },
        payload,
        ..Default::default()
    }
}

fn interactive_action_op(sid: &str, action_id: &str, rev: i64, params: Value) -> AppOperation {
    state_op(
        "interactive.action",
        sid,
        json!({
            "actionId": action_id,
            "stateRevision": rev,
            "params": params,
        }),
    )
}

fn with_ai_actors(mut interactive: Value, actors: &[&str]) -> Value {
    interactive["aiActors"] = json!(actors);
    // Probe gates on currentPlayer ∈ aiActors; fixtures without a turn key need one.
    let has_cp = interactive["stateSchema"]
        .as_array()
        .map(|s| {
            s.iter()
                .any(|e| e.get("key").and_then(|k| k.as_str()) == Some("currentPlayer"))
        })
        .unwrap_or(false);
    if !has_cp {
        let actor = actors.first().copied().unwrap_or("user");
        interactive["stateSchema"]
            .as_array_mut()
            .expect("stateSchema")
            .push(json!({
                "key": "currentPlayer",
                "type": "string",
                "initialValue": actor
            }));
    }
    interactive
}

fn assert_quiz_rejection_hides_answers(rej: &AiActionRejection) {
    assert!(rej.fresh_context["publicState"].get("answers").is_none());
    let encoded = serde_json::to_string(rej).unwrap();
    assert!(
        !encoded.contains("[1,0,0]")
            && !encoded.contains("[1, 0, 0]")
            && !encoded.contains("\"answers\":[1"),
        "rejection must not leak quiz answers: {encoded}"
    );
    assert!(
        !rej.safe_message.contains("answers") && !rej.safe_message.contains("[1"),
        "safe_message must not mention hidden answers: {}",
        rej.safe_message
    );
}

// ---------------------------------------------------------------------------
// Admission and first view
// ---------------------------------------------------------------------------

#[test]
fn first_view_initializes_game_and_offers_moves_to_the_first_player() {
    let mut e = env();
    let sid = create(&mut e, fixture_json("tic-tac-toe"));

    let view = get_view(&mut e.db, &sid).unwrap();
    assert!(view.reinitialized);
    assert_eq!(view.application_id, "game-tictactoe");
    assert_eq!(view.state_revision, 2);
    assert_eq!(view.actor.as_deref(), Some("X"));
    assert_eq!(view.waiting_for, None);
    assert_eq!(view.status, "playing");
    assert_eq!(view.seq, 1);
    assert_eq!(
        view.state["board"],
        json!(["", "", "", "", "", "", "", "", ""])
    );
    let ids: Vec<&str> = view
        .legal_actions
        .iter()
        .map(|a| a.action_id.as_str())
        .collect();
    assert!(ids.contains(&"move") && ids.contains(&"reset"));
    assert!(
        view.legal_actions
            .iter()
            .find(|a| a.action_id == "move")
            .unwrap()
            .requires_parameters
    );

    let (state, rev) = raw_state(&e.db, &sid);
    assert_eq!(rev, 2);
    assert_eq!(state["currentPlayer"], "X");

    // A second view does not re-initialize or write.
    let again = get_view(&mut e.db, &sid).unwrap();
    assert!(!again.reinitialized);
    assert_eq!(again.state_revision, 2);
    assert_eq!(log_count(&e.db, &sid), 1);
}

#[test]
fn surfaces_with_failing_self_tests_or_unknown_fields_are_not_created() {
    let mut e = env();
    let mut failing = fixture_json("tic-tac-toe");
    failing["testCases"][0]["expectedStateSubset"]["currentPlayer"] = json!("X");
    let mut unknown = fixture_json("tic-tac-toe");
    unknown["onLoad"] = json!("fetch('https://evil')");
    let mut unknown_effect = fixture_json("tic-tac-toe");
    unknown_effect["actions"][0]["effects"][0]["script"] = json!("x");

    for bad in [failing, unknown, unknown_effect] {
        let res = create_inline_surface(
            &mut e.db,
            &e.conv,
            None,
            None,
            "Bad",
            &surface_def(bad),
            &[],
        );
        match res {
            Err(DbError::Invalid(msg)) => assert!(msg.contains("interactive"), "{msg}"),
            other => panic!("expected admission failure, got {other:?}"),
        }
    }
    let count: i64 =
        e.db.conn()
            .query_row(
                "SELECT COUNT(*) FROM surfaces WHERE conversation_id = ?1",
                [&e.conv],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(count, 0);
}

// ---------------------------------------------------------------------------
// Dispatch, revisions, idempotency
// ---------------------------------------------------------------------------

#[test]
fn legal_move_is_persisted_and_advances_the_revision() {
    let mut e = env();
    let sid = create(&mut e, fixture_json("tic-tac-toe"));
    let v0 = get_view(&mut e.db, &sid).unwrap();

    let v1 = dispatch(
        &mut e.db,
        &sid,
        v0.state_revision,
        "ev-1",
        "move",
        &json!({"index": 4}),
        Origin::User,
    )
    .unwrap();
    assert_eq!(v1.state_revision, v0.state_revision + 1);
    assert_eq!(v1.seq, 2);
    assert!(!v1.duplicate);
    assert_eq!(v1.actor.as_deref(), Some("O"));
    assert_eq!(v1.state["board"][4], "X");

    let (state, rev) = raw_state(&e.db, &sid);
    assert_eq!(rev, v1.state_revision);
    assert_eq!(state["board"][4], "X");
    assert_eq!(state["currentPlayer"], "O");
    assert_eq!(state["turn"], 2);
    let h = history(&e.db, &sid).unwrap();
    assert_eq!(h.len(), 2);
    assert_eq!(h[1].action_id, "move");
    assert_eq!(h[1].actor, "X");
    assert_eq!(h[1].params, json!({"index": 4}));
}

#[test]
fn stale_duplicate_and_illegal_dispatches_do_not_mutate_state() {
    let mut e = env();
    let sid = create(&mut e, fixture_json("tic-tac-toe"));
    let v0 = get_view(&mut e.db, &sid).unwrap();
    let v1 = dispatch(
        &mut e.db,
        &sid,
        v0.state_revision,
        "ev-1",
        "move",
        &json!({"index": 4}),
        Origin::User,
    )
    .unwrap();
    let snapshot = raw_state_json(&e.db, &sid);
    let rows = log_count(&e.db, &sid);

    // Stale revision.
    let stale = dispatch(
        &mut e.db,
        &sid,
        v0.state_revision,
        "ev-2",
        "move",
        &json!({"index": 0}),
        Origin::User,
    );
    assert!(matches!(stale, Err(DbError::Conflict(_))), "{stale:?}");
    assert_eq!(raw_state_json(&e.db, &sid), snapshot);
    assert_eq!(log_count(&e.db, &sid), rows);

    // Duplicate event id: no second application, even with a different payload.
    let dup = dispatch(
        &mut e.db,
        &sid,
        v1.state_revision,
        "ev-1",
        "move",
        &json!({"index": 0}),
        Origin::User,
    )
    .unwrap();
    assert!(dup.duplicate);
    assert_eq!(dup.state_revision, v1.state_revision);
    assert_eq!(dup.state["board"][0], "");
    assert_eq!(raw_state_json(&e.db, &sid), snapshot);
    assert_eq!(log_count(&e.db, &sid), rows);

    // Illegal: occupied cell, out-of-range, unknown action, unknown param.
    for (action, params) in [
        ("move", json!({"index": 4})),
        ("move", json!({"index": 9})),
        ("move", json!({"index": -1})),
        ("move", json!({"index": 1, "extra": 1})),
        ("teleport", json!({})),
    ] {
        let res = dispatch(
            &mut e.db,
            &sid,
            v1.state_revision,
            "ev-bad",
            action,
            &params,
            Origin::User,
        );
        assert!(
            matches!(res, Err(DbError::Invalid(_))),
            "{action} {params}: {res:?}"
        );
        assert_eq!(raw_state_json(&e.db, &sid), snapshot);
        assert_eq!(log_count(&e.db, &sid), rows);
    }

    // A rejected event id was never recorded, so it can be used for a real move.
    let ok = dispatch(
        &mut e.db,
        &sid,
        v1.state_revision,
        "ev-bad",
        "move",
        &json!({"index": 0}),
        Origin::User,
    )
    .unwrap();
    assert!(!ok.duplicate);
    assert_eq!(ok.state["board"][0], "O");

    // Engine-reserved event ids are rejected.
    let reserved = dispatch(
        &mut e.db,
        &sid,
        ok.state_revision,
        "__init-1",
        "move",
        &json!({"index": 1}),
        Origin::User,
    );
    assert!(reserved.is_err());
}

// ---------------------------------------------------------------------------
// Owned-key write guards
// ---------------------------------------------------------------------------

#[test]
fn renderer_saves_cannot_change_engine_owned_keys() {
    let mut e = env();
    let sid = create(&mut e, fixture_json("tic-tac-toe"));
    let v0 = get_view(&mut e.db, &sid).unwrap();
    let (state, rev) = raw_state(&e.db, &sid);

    let mut forged = state["board"].clone();
    forged[0] = json!("X");
    for patch in [
        json!({ "board": forged }),
        json!({ "currentPlayer": "O" }),
        json!({ "winner": "X" }),
        json!({ "phase": "done" }),
        json!({ RNG_STATE_KEY: { "seed": "1", "state": "1" } }),
    ] {
        let res = save_surface_state_user_cas(&mut e.db, &sid, Some(rev), &patch);
        assert!(
            matches!(res, Err(DbError::Invalid(ref m)) if m.contains("owned")),
            "{patch}: {res:?}"
        );
    }
    assert_eq!(raw_state(&e.db, &sid), (state.clone(), rev));

    // Writing the current value of an owned key plus a non-owned key is allowed.
    let (saved, next_rev) = save_surface_state_user_cas(
        &mut e.db,
        &sid,
        Some(rev),
        &json!({ "board": state["board"], "note": "hello" }),
    )
    .unwrap();
    assert_eq!(next_rev, rev + 1);
    assert_eq!(saved["note"], "hello");
    assert_eq!(saved["board"], state["board"]);

    // Non-owned keys survive dispatch and show in the view.
    let v1 = dispatch(
        &mut e.db,
        &sid,
        next_rev,
        "ev-1",
        "move",
        &json!({"index": 0}),
        Origin::User,
    )
    .unwrap();
    assert_eq!(v1.state["note"], "hello");
    assert_eq!(raw_state(&e.db, &sid).0["note"], "hello");
    assert!(v0.state.get("note").is_none());
}

#[test]
fn agent_state_operations_cannot_touch_engine_owned_keys() {
    let mut e = env();
    let sid = create(&mut e, fixture_json("tic-tac-toe"));
    get_view(&mut e.db, &sid).unwrap();
    let before = raw_state_json(&e.db, &sid);

    for (op_type, payload) in [
        (
            "state.patch",
            json!({ "state": { "board": ["X","X","X","","","","","",""] } }),
        ),
        ("state.set", json!({ "key": "winner", "value": "X" })),
        (
            "state.set",
            json!({ "state": { "note": "ok", "status": "won" } }),
        ),
    ] {
        let txn = create_transaction(
            &mut e.db,
            Some(&e.conv),
            None,
            None,
            "agent edit",
            &[state_op(op_type, &sid, payload.clone())],
            false,
        )
        .unwrap();
        let res = apply_transaction(&mut e.db, &txn.id).unwrap();
        assert_eq!(res.transaction.status, "failed", "{op_type} {payload}");
        assert!(
            res.conflicts
                .iter()
                .any(|c| c.contains("owned by the interactive rules engine")),
            "{:?}",
            res.conflicts
        );
        assert_eq!(raw_state_json(&e.db, &sid), before);
    }

    // Non-owned keys remain writable by the agent.
    let txn = create_transaction(
        &mut e.db,
        Some(&e.conv),
        None,
        None,
        "note",
        &[state_op(
            "state.patch",
            &sid,
            json!({ "state": { "note": "agent" } }),
        )],
        false,
    )
    .unwrap();
    let res = apply_transaction(&mut e.db, &txn.id).unwrap();
    assert_eq!(res.transaction.status, "applied", "{:?}", res.conflicts);
    assert_eq!(raw_state(&e.db, &sid).0["note"], "agent");
}

#[test]
fn undoing_an_agent_transaction_does_not_rewind_moves_made_after_it() {
    let mut e = env();
    let sid = create(&mut e, fixture_json("tic-tac-toe"));
    get_view(&mut e.db, &sid).unwrap();

    // Agent edits a component and a non-owned key in one transaction.
    let txn = create_transaction(
        &mut e.db,
        Some(&e.conv),
        None,
        None,
        "retitle",
        &[
            {
                let mut op = state_op(
                    "component.update_props",
                    &sid,
                    json!({ "componentId": "title", "props": { "text": "Renamed" } }),
                );
                op.target.component_id = Some("title".into());
                op
            },
            state_op("state.patch", &sid, json!({ "state": { "note": "agent" } })),
        ],
        false,
    )
    .unwrap();
    let applied = apply_transaction(&mut e.db, &txn.id).unwrap();
    assert_eq!(
        applied.transaction.status, "applied",
        "{:?}",
        applied.conflicts
    );

    let v = get_view(&mut e.db, &sid).unwrap();
    let v = dispatch(
        &mut e.db,
        &sid,
        v.state_revision,
        "ev-1",
        "move",
        &json!({"index": 4}),
        Origin::User,
    )
    .unwrap();
    assert_eq!(v.state["board"][4], "X");

    undo_transaction(&mut e.db, &txn.id).unwrap();
    let after = get_view(&mut e.db, &sid).unwrap();
    let (state, _) = raw_state(&e.db, &sid);
    assert_eq!(
        state["board"][4], "X",
        "transaction undo rewound engine-owned state"
    );
    assert_eq!(state["currentPlayer"], "O");
    assert!(
        state.get("note").is_none(),
        "agent's non-owned change is reverted"
    );
    assert_eq!(after.state["board"][4], "X");
    assert!(reconstruct(&e.db, &sid, after.seq).is_ok());
}

// ---------------------------------------------------------------------------
// Randomness
// ---------------------------------------------------------------------------

#[test]
fn rng_advances_once_per_successful_random_action_and_never_on_failure() {
    let mut e = env();
    let def = get_fixture("card-deck").unwrap();
    let sid = create(&mut e, serde_json::to_value(&def).unwrap());
    let v0 = get_view(&mut e.db, &sid).unwrap();
    assert!(v0.state.get(RNG_STATE_KEY).is_none());
    let (s0, _) = raw_state(&e.db, &sid);
    assert_eq!(
        s0[RNG_STATE_KEY],
        def.initial_instance_state().unwrap()[RNG_STATE_KEY]
    );

    let v1 = act(&mut e, &sid, "shuffle-1", "shuffle", json!({}));
    let (s1, _) = raw_state(&e.db, &sid);
    // Exactly one engine application from the previous persisted state.
    let mut expected1 = owned_of(&def, &s0);
    def.execute_action(&mut expected1, "shuffle", &json!({}), "player")
        .unwrap();
    assert_eq!(owned_of(&def, &s1), expected1);
    assert_ne!(s1[RNG_STATE_KEY], s0[RNG_STATE_KEY]);
    assert!(v1.state.get(RNG_STATE_KEY).is_none());

    let v2 = act(&mut e, &sid, "shuffle-2", "shuffle", json!({}));
    let (s2, _) = raw_state(&e.db, &sid);
    let mut expected2 = owned_of(&def, &s1);
    def.execute_action(&mut expected2, "shuffle", &json!({}), "player")
        .unwrap();
    assert_eq!(owned_of(&def, &s2), expected2);
    assert!(v2.state.get(RNG_STATE_KEY).is_none());

    // If the RNG were reset per action, shuffle #2 would equal a fresh-seed shuffle of deck #1.
    let mut reseeded = owned_of(&def, &s1);
    reseeded[RNG_STATE_KEY] = s0[RNG_STATE_KEY].clone();
    def.execute_action(&mut reseeded, "shuffle", &json!({}), "player")
        .unwrap();
    assert_ne!(
        s2["deck"], reseeded["deck"],
        "second shuffle must continue the RNG stream"
    );
    assert_ne!(s2[RNG_STATE_KEY], s1[RNG_STATE_KEY]);

    // Failed action: RNG untouched.
    let rev = raw_state(&e.db, &sid).1;
    let failed = dispatch(
        &mut e.db,
        &sid,
        rev,
        "discard-bad",
        "discard",
        &json!({"index": 0}),
        Origin::User,
    );
    assert!(failed.is_err());
    assert_eq!(raw_state(&e.db, &sid).0, s2);

    // Non-random action: RNG untouched.
    act(&mut e, &sid, "draw-1", "draw", json!({}));
    let (s3, _) = raw_state(&e.db, &sid);
    assert_eq!(s3[RNG_STATE_KEY], s2[RNG_STATE_KEY]);
    assert_eq!(s3["hand"], json!([s2["deck"][0].clone()]));
}

#[test]
fn rng_state_is_hidden_and_not_advanced_by_rejected_2048_slides() {
    let mut e = env();
    let def = get_fixture("2048").unwrap();
    let sid = create(&mut e, serde_json::to_value(&def).unwrap());
    get_view(&mut e.db, &sid).unwrap();

    let mut rng_values = vec![raw_state(&e.db, &sid).0[RNG_STATE_KEY].clone()];
    let mut n = 0;
    for dir in ["left", "left", "up", "up", "right", "down", "left", "up"] {
        let before = raw_state(&e.db, &sid);
        n += 1;
        match dispatch(
            &mut e.db,
            &sid,
            before.1,
            &format!("s-{n}"),
            "slide",
            &json!({"direction": dir}),
            Origin::User,
        ) {
            Ok(view) => {
                assert!(view.state.get(RNG_STATE_KEY).is_none());
                let after = raw_state(&e.db, &sid).0;
                let mut expected = owned_of(&def, &before.0);
                def.execute_action(&mut expected, "slide", &json!({"direction": dir}), "player")
                    .unwrap();
                assert_eq!(owned_of(&def, &after), expected);
                let rng = after[RNG_STATE_KEY].clone();
                assert!(
                    !rng_values.contains(&rng),
                    "RNG must move forward on every spawn"
                );
                rng_values.push(rng);
            }
            Err(_) => assert_eq!(raw_state(&e.db, &sid), before, "rejected slide wrote state"),
        }
    }
    assert!(rng_values.len() >= 4, "expected several successful slides");
}

// ---------------------------------------------------------------------------
// Replay
// ---------------------------------------------------------------------------

fn assert_replay_matches(e: &mut Env, sid: &str, captured: &[(i64, Value)]) {
    let before_state = raw_state_json(&e.db, sid);
    let before_rows = log_count(&e.db, sid);
    for (seq, owned) in captured {
        let rebuilt =
            reconstruct(&e.db, sid, *seq).unwrap_or_else(|err| panic!("seq {seq}: {err}"));
        assert_eq!(&rebuilt, owned, "replay diverged at seq {seq}");
        let public = replay_view(&e.db, sid, *seq).unwrap();
        assert!(public.get(RNG_STATE_KEY).is_none());
    }
    let last = captured.last().unwrap().0;
    assert!(matches!(
        reconstruct(&e.db, sid, last + 1),
        Err(DbError::NotFound(_))
    ));
    assert_eq!(
        raw_state_json(&e.db, sid),
        before_state,
        "replay must not write state"
    );
    assert_eq!(
        log_count(&e.db, sid),
        before_rows,
        "replay must not write log rows"
    );
}

#[test]
fn chess_game_with_castling_replays_exactly_at_every_step() {
    let mut e = env();
    let def = get_fixture("chess").unwrap();
    let sid = create(&mut e, serde_json::to_value(&def).unwrap());
    let v0 = get_view(&mut e.db, &sid).unwrap();
    let mut captured = vec![(v0.seq, owned_of(&def, &raw_state(&e.db, &sid).0))];

    let moves = [
        ("e2", "e4"),
        ("e7", "e5"),
        ("g1", "f3"),
        ("b8", "c6"),
        ("f1", "c4"),
        ("f8", "c5"),
        ("e1", "g1"),
    ];
    for (i, (from, to)) in moves.iter().enumerate() {
        let v = act(
            &mut e,
            &sid,
            &format!("m-{i}"),
            "move",
            json!({"from": from, "to": to}),
        );
        captured.push((v.seq, owned_of(&def, &raw_state(&e.db, &sid).0)));
    }
    let last = &captured.last().unwrap().1;
    let board = &last["chess"]["board"];
    // Rank 1 is the last row: king on g1, rook on f1.
    assert_eq!(board[7][6], "K");
    assert_eq!(board[7][5], "R");
    assert_eq!(last["currentPlayer"], "black");

    assert_replay_matches(&mut e, &sid, &captured);
}

#[test]
fn chess_resign_and_claim_draw_dispatch_through_fixture() {
    let mut e = env();
    let sid = create(&mut e, fixture_json("chess"));
    let _ = get_view(&mut e.db, &sid).unwrap();

    let rev = raw_state(&e.db, &sid).1;
    let claim = dispatch(
        &mut e.db,
        &sid,
        rev,
        "claim-early",
        "claimDraw",
        &json!({}),
        Origin::User,
    );
    assert!(
        claim.is_err(),
        "claimDraw must fail when nothing is claimable: {claim:?}"
    );
    assert_eq!(raw_state(&e.db, &sid).1, rev);

    let resigned = act(&mut e, &sid, "resign-1", "resign", json!({}));
    assert_eq!(resigned.status, "resigned");
    assert_eq!(resigned.state["winner"], "black");
    assert_eq!(resigned.state["chess"]["status"], "resigned");
    assert_eq!(resigned.state["chess"]["winner"], "black");
    assert!(resigned
        .legal_actions
        .iter()
        .any(|a| a.action_id == "reset"));
    // Parameterless endgame actions are gated by the playing guard.
    assert!(!resigned
        .legal_actions
        .iter()
        .any(|a| a.action_id == "resign" || a.action_id == "claimDraw"));
    let rev_after = resigned.state_revision;
    assert!(dispatch(
        &mut e.db,
        &sid,
        rev_after,
        "resign-again",
        "resign",
        &json!({}),
        Origin::User,
    )
    .is_err());
}

#[test]
fn chess_claim_draw_succeeds_after_threefold_via_dispatch() {
    let mut e = env();
    let sid = create(&mut e, fixture_json("chess"));
    let _ = get_view(&mut e.db, &sid).unwrap();

    // Knight shuffle to threefold repetition (claimable, not automatic).
    let moves = [
        ("g1", "f3"),
        ("g8", "f6"),
        ("f3", "g1"),
        ("f6", "g8"),
        ("g1", "f3"),
        ("g8", "f6"),
        ("f3", "g1"),
        ("f6", "g8"),
    ];
    for (i, (from, to)) in moves.iter().enumerate() {
        act(
            &mut e,
            &sid,
            &format!("tf-{i}"),
            "move",
            json!({"from": from, "to": to}),
        );
    }
    let view = get_view(&mut e.db, &sid).unwrap();
    assert_eq!(view.status, "playing");
    assert_eq!(view.state["chess"]["claimableDraw"], "threefold");

    let claimed = act(&mut e, &sid, "claim-tf", "claimDraw", json!({}));
    assert_eq!(claimed.status, "draw_threefold");
    assert_eq!(claimed.state["chess"]["status"], "draw_threefold");
    assert!(claimed.state["chess"]["claimableDraw"].is_null());
    assert!(claimed.legal_actions.iter().any(|a| a.action_id == "reset"));
}

#[test]
fn random_card_game_replays_exactly_at_every_step() {
    let mut e = env();
    let def = get_fixture("card-deck").unwrap();
    let sid = create(&mut e, serde_json::to_value(&def).unwrap());
    let v0 = get_view(&mut e.db, &sid).unwrap();
    let mut captured = vec![(v0.seq, owned_of(&def, &raw_state(&e.db, &sid).0))];
    for (i, (action, params)) in [
        ("shuffle", json!({})),
        ("draw", json!({})),
        ("shuffle", json!({})),
        ("draw", json!({})),
        ("discard", json!({"index": 0})),
        ("shuffle", json!({})),
    ]
    .into_iter()
    .enumerate()
    {
        let v = act(&mut e, &sid, &format!("c-{i}"), action, params);
        captured.push((v.seq, owned_of(&def, &raw_state(&e.db, &sid).0)));
    }
    assert_replay_matches(&mut e, &sid, &captured);
}

// ---------------------------------------------------------------------------
// Undo
// ---------------------------------------------------------------------------

#[test]
fn undo_steps_back_repeatedly_and_replay_still_verifies() {
    let mut e = env();
    let def = get_fixture("tic-tac-toe").unwrap();
    let sid = create(&mut e, serde_json::to_value(&def).unwrap());
    let v0 = get_view(&mut e.db, &sid).unwrap();
    let s0 = owned_of(&def, &raw_state(&e.db, &sid).0);
    let mut captured = vec![(v0.seq, s0.clone())];
    let mut states = vec![s0];
    for (i, idx) in [0, 4, 8].into_iter().enumerate() {
        let v = act(
            &mut e,
            &sid,
            &format!("m-{i}"),
            "move",
            json!({"index": idx}),
        );
        let s = owned_of(&def, &raw_state(&e.db, &sid).0);
        captured.push((v.seq, s.clone()));
        states.push(s);
    }

    let rev = raw_state(&e.db, &sid).1;
    let stale = undo(&mut e.db, &sid, rev - 1, "u-stale");
    assert!(matches!(stale, Err(DbError::Conflict(_))), "{stale:?}");
    assert_eq!(raw_state(&e.db, &sid).1, rev);

    let u1 = undo(&mut e.db, &sid, rev, "u-1").unwrap();
    assert_eq!(u1.state_revision, rev + 1);
    assert_eq!(owned_of(&def, &raw_state(&e.db, &sid).0), states[2]);
    assert_eq!(u1.actor.as_deref(), Some("X"));
    captured.push((u1.seq, states[2].clone()));

    let u2 = undo(&mut e.db, &sid, u1.state_revision, "u-2").unwrap();
    assert_eq!(owned_of(&def, &raw_state(&e.db, &sid).0), states[1]);
    assert_eq!(
        u2.state["board"],
        json!(["X", "", "", "", "", "", "", "", ""])
    );
    captured.push((u2.seq, states[1].clone()));

    // A new move after undo continues from the undone state.
    let v = act(&mut e, &sid, "m-after", "move", json!({"index": 2}));
    assert_eq!(v.state["board"][2], "O");
    captured.push((v.seq, owned_of(&def, &raw_state(&e.db, &sid).0)));

    let u3 = undo(&mut e.db, &sid, v.state_revision, "u-3").unwrap();
    assert_eq!(owned_of(&def, &raw_state(&e.db, &sid).0), states[1]);
    captured.push((u3.seq, states[1].clone()));

    let u4 = undo(&mut e.db, &sid, u3.state_revision, "u-4").unwrap();
    assert_eq!(owned_of(&def, &raw_state(&e.db, &sid).0), states[0]);
    captured.push((u4.seq, states[0].clone()));

    let nothing = undo(&mut e.db, &sid, u4.state_revision, "u-5");
    assert!(matches!(nothing, Err(DbError::Invalid(_))), "{nothing:?}");

    assert_replay_matches(&mut e, &sid, &captured);
}

#[test]
fn duplicate_undo_event_id_is_soft_idempotent() {
    let mut e = env();
    let sid = create(&mut e, fixture_json("tic-tac-toe"));
    get_view(&mut e.db, &sid).unwrap();
    act(&mut e, &sid, "m-1", "move", json!({"index": 4}));
    act(&mut e, &sid, "m-2", "move", json!({"index": 0}));

    let (_, rev) = raw_state(&e.db, &sid);
    let rows_before = log_count(&e.db, &sid);

    let first = undo(&mut e.db, &sid, rev, "u-same").unwrap();
    assert!(!first.duplicate);
    assert_eq!(first.state_revision, rev + 1);
    assert_eq!(first.state["board"][0], "");
    assert_eq!(first.state["board"][4], "X");
    let after_first = raw_state_json(&e.db, &sid);
    let rows_after_first = log_count(&e.db, &sid);
    assert_eq!(rows_after_first, rows_before + 1);
    assert_eq!(after_first.1, first.state_revision);

    // Retry with the same event_id must soft-return, not UNIQUE-fail or re-apply.
    let dup = undo(&mut e.db, &sid, first.state_revision, "u-same").unwrap();
    assert!(dup.duplicate);
    assert_eq!(dup.state_revision, first.state_revision);
    assert_eq!(dup.seq, first.seq);
    assert_eq!(dup.state["board"], first.state["board"]);
    assert_eq!(raw_state_json(&e.db, &sid), after_first);
    assert_eq!(log_count(&e.db, &sid), rows_after_first);
}

// ---------------------------------------------------------------------------
// Corruption
// ---------------------------------------------------------------------------

#[test]
fn corrupted_owned_state_fails_closed_without_being_overwritten() {
    let mut e = env();
    let sid = create(&mut e, fixture_json("tic-tac-toe"));
    let v = get_view(&mut e.db, &sid).unwrap();
    let mut state = raw_state(&e.db, &sid).0;
    state["board"] = json!(5);
    write_raw_state(&e.db, &sid, &state);
    let corrupted = raw_state_json(&e.db, &sid);
    let rows = log_count(&e.db, &sid);

    assert!(get_view(&mut e.db, &sid).is_err());
    let d = dispatch(
        &mut e.db,
        &sid,
        v.state_revision,
        "ev-1",
        "move",
        &json!({"index": 0}),
        Origin::User,
    );
    assert!(d.is_err());
    assert!(undo(&mut e.db, &sid, v.state_revision, "u-1").is_err());
    assert_eq!(raw_state_json(&e.db, &sid), corrupted);
    assert_eq!(log_count(&e.db, &sid), rows);
}

#[test]
fn tampered_action_log_is_detected_on_replay() {
    let mut e = env();
    let sid = create(&mut e, fixture_json("tic-tac-toe"));
    get_view(&mut e.db, &sid).unwrap();
    act(&mut e, &sid, "m-1", "move", json!({"index": 4}));
    act(&mut e, &sid, "m-2", "move", json!({"index": 0}));
    assert!(reconstruct(&e.db, &sid, 3).is_ok());

    e.db.conn()
        .execute(
            "UPDATE interactive_action_log SET params_json = '{\"index\":5}' WHERE surface_id = ?1 AND seq = 2",
            [&sid],
        )
        .unwrap();
    for seq in [2, 3] {
        match reconstruct(&e.db, &sid, seq) {
            Err(DbError::Invalid(msg)) => assert!(msg.contains("diverged"), "{msg}"),
            other => panic!("tampered log must not replay at seq {seq}: {other:?}"),
        }
    }
    assert!(
        reconstruct(&e.db, &sid, 1).is_ok(),
        "checkpoint before tamper is intact"
    );

    // Tampered checkpoint snapshot is detected too.
    e.db.conn()
        .execute(
            "UPDATE interactive_action_log SET snapshot_json = replace(snapshot_json, '\"X\"', '\"O\"') WHERE surface_id = ?1 AND seq = 1",
            [&sid],
        )
        .unwrap();
    assert!(reconstruct(&e.db, &sid, 1).is_err());
}

#[test]
fn replay_view_preserves_live_non_owned_keys() {
    let mut e = env();
    let sid = create(&mut e, fixture_json("tic-tac-toe"));
    let v0 = get_view(&mut e.db, &sid).unwrap();
    let v1 = dispatch(
        &mut e.db,
        &sid,
        v0.state_revision,
        "m-1",
        "move",
        &json!({"index": 4}),
        Origin::User,
    )
    .unwrap();

    let (mut state, _rev) = raw_state(&e.db, &sid);
    state["uiTheme"] = json!("dark");
    write_raw_state(&e.db, &sid, &state);

    let replayed = replay_view(&e.db, &sid, v1.seq).unwrap();
    assert_eq!(replayed["board"][4], "X");
    assert_eq!(replayed["uiTheme"], "dark");
    assert!(replayed.get(RNG_STATE_KEY).is_none());
}

#[test]
fn definition_change_adopts_valid_state_and_blocks_stale_dispatch() {
    let mut e = env();
    let sid = create(&mut e, fixture_json("tic-tac-toe"));
    let v0 = get_view(&mut e.db, &sid).unwrap();
    let v1 = dispatch(
        &mut e.db,
        &sid,
        v0.state_revision,
        "m-1",
        "move",
        &json!({"index": 4}),
        Origin::User,
    )
    .unwrap();

    let surface = get_surface(&e.db, &sid).unwrap();
    let mut def = surface.definition.clone();
    def["title"] = json!("Renamed");
    update_surface_definition(&mut e.db, &sid, &def, "rename", None).unwrap();

    // The first dispatch after a definition change must not apply on top of an unverified adoption.
    let d = dispatch(
        &mut e.db,
        &sid,
        v1.state_revision,
        "m-2",
        "move",
        &json!({"index": 0}),
        Origin::User,
    );
    assert!(matches!(d, Err(DbError::Conflict(_))), "{d:?}");
    let view = get_view(&mut e.db, &sid).unwrap();
    assert_eq!(
        view.state["board"][4], "X",
        "valid state is adopted, not reset"
    );
    let h = history(&e.db, &sid).unwrap();
    assert_eq!(h.last().unwrap().action_id, "__adopt");
    assert!(h.last().unwrap().checkpoint);
    let v2 = dispatch(
        &mut e.db,
        &sid,
        view.state_revision,
        "m-2",
        "move",
        &json!({"index": 0}),
        Origin::User,
    )
    .unwrap();
    assert_eq!(v2.state["board"][0], "O");
    assert_eq!(reconstruct(&e.db, &sid, v2.seq).unwrap()["board"][0], "O");
}

// ---------------------------------------------------------------------------
// AI actors and hidden state
// ---------------------------------------------------------------------------

#[test]
fn ai_actor_moves_only_on_its_turn_and_user_cannot_act_for_it() {
    let mut e = env();
    let mut ttt = fixture_json("tic-tac-toe");
    ttt["aiActors"] = json!(["O"]);
    let sid = create(&mut e, ttt);
    let v0 = get_view(&mut e.db, &sid).unwrap();

    let early_ai = dispatch(
        &mut e.db,
        &sid,
        v0.state_revision,
        "ai-0",
        "move",
        &json!({"index": 0}),
        Origin::Ai,
    );
    assert!(early_ai.is_err(), "AI must not move on the user's turn");

    let v1 = dispatch(
        &mut e.db,
        &sid,
        v0.state_revision,
        "u-1",
        "move",
        &json!({"index": 4}),
        Origin::User,
    )
    .unwrap();
    assert_eq!(v1.actor, None);
    assert_eq!(v1.waiting_for.as_deref(), Some("O"));
    // Legal actions describe the waiting AI actor's options; the user still cannot use them.
    assert!(v1.legal_actions.iter().any(|a| a.action_id == "move"));

    let rows = log_count(&e.db, &sid);
    for action in ["move", "reset"] {
        let params = if action == "move" {
            json!({"index": 0})
        } else {
            json!({})
        };
        let user = dispatch(
            &mut e.db,
            &sid,
            v1.state_revision,
            "u-2",
            action,
            &params,
            Origin::User,
        );
        assert!(
            matches!(user, Err(DbError::Invalid(ref m)) if m.contains("Waiting for O")),
            "{user:?}"
        );
    }
    assert_eq!(log_count(&e.db, &sid), rows);

    let v2 = dispatch(
        &mut e.db,
        &sid,
        v1.state_revision,
        "ai-1",
        "move",
        &json!({"index": 0}),
        Origin::Ai,
    )
    .unwrap();
    assert_eq!(v2.state["board"][0], "O");
    assert_eq!(v2.actor.as_deref(), Some("X"));
    let h = history(&e.db, &sid).unwrap();
    assert_eq!(h.last().unwrap().actor, "O");

    let ai_again = dispatch(
        &mut e.db,
        &sid,
        v2.state_revision,
        "ai-2",
        "move",
        &json!({"index": 1}),
        Origin::Ai,
    );
    assert!(ai_again.is_err());
}

#[test]
fn probe_ai_action_accepts_legal_move_on_ai_turn() {
    let mut e = env();
    let mut ttt = fixture_json("tic-tac-toe");
    ttt["aiActors"] = json!(["O"]);
    let sid = create(&mut e, ttt);
    let _ = get_view(&mut e.db, &sid).unwrap();
    let v1 = act(&mut e, &sid, "u-center", "move", json!({"index": 4}));
    assert_eq!(v1.waiting_for.as_deref(), Some("O"));
    let rev = v1.state_revision;
    let rows = log_count(&e.db, &sid);

    assert!(probe_ai_action(&mut e.db, &sid, rev, "move", &json!({"index": 0}), 0).is_ok());
    assert_eq!(log_count(&e.db, &sid), rows, "probe must not mutate");
    assert_eq!(raw_state(&e.db, &sid).1, rev);
    // Confirm the probed move still applies afterward.
    let v2 = dispatch(
        &mut e.db,
        &sid,
        rev,
        "ai-probe-ok",
        "move",
        &json!({"index": 0}),
        Origin::Ai,
    )
    .unwrap();
    assert_eq!(v2.state["board"][0], "O");
}

#[test]
fn probe_ai_action_rejects_illegal_without_leaking_quiz_answers() {
    let mut e = env();
    let sid = create(&mut e, with_ai_actors(fixture_json("quiz"), &["user"]));
    let _ = get_view(&mut e.db, &sid).unwrap();
    // aiActors=["user"] so only Origin::Ai may answer; finish the quiz then probe.
    for (i, choice) in [1, 0, 0].into_iter().enumerate() {
        let rev = raw_state(&e.db, &sid).1;
        dispatch(
            &mut e.db,
            &sid,
            rev,
            &format!("q-{i}"),
            "answer",
            &json!({"choice": choice}),
            Origin::Ai,
        )
        .unwrap_or_else(|err| panic!("answer {choice} failed: {err}"));
    }
    let done = get_view(&mut e.db, &sid).unwrap();
    assert_eq!(done.status, "completed");
    let rev = done.state_revision;
    let rows = log_count(&e.db, &sid);

    let err = probe_ai_action(&mut e.db, &sid, rev, "answer", &json!({"choice": 0}), 1)
        .expect_err("completed quiz answer must be illegal");
    assert_eq!(err.reason_code, "illegal");
    assert_eq!(err.attempt_index, 1);
    assert_eq!(err.action_id.as_deref(), Some("answer"));
    assert!(err.attempt_id.contains("-1"));
    assert!(err.budget_remaining < MAX_REPAIR_ATTEMPTS as u32);
    assert_quiz_rejection_hides_answers(&err);
    assert_eq!(log_count(&e.db, &sid), rows);
    assert_eq!(raw_state(&e.db, &sid).1, rev);
}

#[test]
fn probe_ai_action_rejects_stale_revision() {
    let mut e = env();
    let mut ttt = fixture_json("tic-tac-toe");
    ttt["aiActors"] = json!(["O"]);
    let sid = create(&mut e, ttt);
    let v0 = get_view(&mut e.db, &sid).unwrap();
    let v1 = act(&mut e, &sid, "u-1", "move", json!({"index": 4}));
    let stale = v0.state_revision;
    assert_ne!(stale, v1.state_revision);

    let err = probe_ai_action(&mut e.db, &sid, stale, "move", &json!({"index": 0}), 0)
        .expect_err("stale revision must fail");
    assert_eq!(err.reason_code, "stale_revision");
    assert!(err.safe_message.to_lowercase().contains("stale"));
    assert_eq!(err.fresh_context["stateRevision"], v1.state_revision);
}

#[test]
fn probe_ai_operations_rejects_wrong_surface() {
    let mut e = env();
    let mut ttt = fixture_json("tic-tac-toe");
    ttt["aiActors"] = json!(["O"]);
    let sid = create(&mut e, ttt);
    let _ = get_view(&mut e.db, &sid).unwrap();
    let v1 = act(&mut e, &sid, "u-1", "move", json!({"index": 4}));
    let mut wrong = interactive_action_op(
        "other-surface",
        "move",
        v1.state_revision,
        json!({"index": 0}),
    );
    wrong.target.surface_id = Some("other-surface".into());

    let err = probe_ai_operations(&mut e.db, &sid, &[wrong], 0)
        .expect_err("mismatched surface must fail");
    assert_eq!(err.reason_code, "wrong_surface");
    assert_eq!(err.action_id.as_deref(), Some("move"));
    // Non-interactive ops are ignored; only the bad interactive.action fails the batch.
    let ignored = state_op("state.patch", &sid, json!({"path": "/note", "value": "x"}));
    let legal = interactive_action_op(&sid, "move", v1.state_revision, json!({"index": 0}));
    assert!(probe_ai_operations(&mut e.db, &sid, &[ignored, legal], 0).is_ok());
}

#[test]
fn ai_action_budget_allows_retry_until_max_repair_attempts() {
    for i in 0..MAX_REPAIR_ATTEMPTS {
        assert!(
            ai_action_budget_allows_retry(i as u32),
            "attempt {i} should still allow retry"
        );
    }
    assert!(!ai_action_budget_allows_retry(MAX_REPAIR_ATTEMPTS as u32));
    assert!(!ai_action_budget_allows_retry(
        MAX_REPAIR_ATTEMPTS as u32 + 5
    ));
}

#[test]
fn new_ai_action_attempt_id_is_unique_and_indexed() {
    let a = new_ai_action_attempt_id(2);
    let b = new_ai_action_attempt_id(2);
    assert!(a.starts_with("ai-act-") && a.ends_with("-2"), "{a}");
    assert!(b.starts_with("ai-act-") && b.ends_with("-2"), "{b}");
    assert_ne!(a, b);
}

#[test]
fn probe_rejection_fresh_context_is_model_safe_for_quiz() {
    let mut e = env();
    let sid = create(&mut e, with_ai_actors(fixture_json("quiz"), &["user"]));
    let _ = get_view(&mut e.db, &sid).unwrap();
    let rev = raw_state(&e.db, &sid).1;

    let err = probe_ai_action(&mut e.db, &sid, rev, "answer", &json!({"choice": 99}), 0)
        .expect_err("out-of-range choice must be illegal");
    assert_eq!(err.reason_code, "illegal");
    assert_quiz_rejection_hides_answers(&err);
    assert_eq!(err.fresh_context["authority"], "rust");
    assert!(err.fresh_context["publicState"].get("answers").is_none());
    assert!(err.fresh_context["modelDefinition"]["stateSchema"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["key"] == "answers" && c["valueRedacted"] == true));
}

#[test]
fn quiz_answers_stay_hidden_while_scoring_uses_them() {
    let mut e = env();
    let sid = create(&mut e, fixture_json("quiz"));
    let v0 = get_view(&mut e.db, &sid).unwrap();
    assert!(v0.state.get("answers").is_none());
    assert_eq!(v0.state["total"], 3);
    assert_eq!(raw_state(&e.db, &sid).0["answers"], json!([1, 0, 0]));

    let v1 = act(&mut e, &sid, "a-1", "answer", json!({"choice": 1}));
    assert!(v1.state.get("answers").is_none());
    assert_eq!(v1.state["score"], 1);
    assert_eq!(v1.state["lastResult"], "correct");
    let v2 = act(&mut e, &sid, "a-2", "answer", json!({"choice": 2}));
    assert_eq!(v2.state["score"], 1);
    assert_eq!(v2.state["lastResult"], "incorrect");
    let v3 = act(&mut e, &sid, "a-3", "answer", json!({"choice": 0}));
    assert_eq!(v3.state["score"], 2);
    assert_eq!(v3.status, "completed");
    let rev = raw_state(&e.db, &sid).1;
    assert!(dispatch(
        &mut e.db,
        &sid,
        rev,
        "a-4",
        "answer",
        &json!({"choice": 0}),
        Origin::User
    )
    .is_err());

    for seq in 1..=v3.seq {
        assert!(replay_view(&e.db, &sid, seq)
            .unwrap()
            .get("answers")
            .is_none());
    }
    // Renderer cannot overwrite hidden answers to cheat.
    let rev = raw_state(&e.db, &sid).1;
    assert!(save_surface_state_user_cas(
        &mut e.db,
        &sid,
        Some(rev),
        &json!({"answers": [0, 0, 0]})
    )
    .is_err());
}

#[test]
fn definition_update_cannot_broaden_interactive_read_policy() {
    let mut e = env();
    let sid = create(&mut e, fixture_json("quiz"));
    let _ = get_view(&mut e.db, &sid).unwrap();
    let _ = act(&mut e, &sid, "a-1", "answer", json!({"choice": 1}));
    assert!(get_view(&mut e.db, &sid)
        .unwrap()
        .state
        .get("answers")
        .is_none());

    let surface = get_surface(&e.db, &sid).unwrap();
    let mut leaked = surface.definition.clone();
    let schema = leaked["interactive"]["stateSchema"]
        .as_array_mut()
        .expect("quiz stateSchema");
    for entry in schema.iter_mut() {
        if entry.get("key").and_then(|k| k.as_str()) == Some("answers") {
            entry.as_object_mut().unwrap().remove("readPolicy");
        }
    }

    let err = update_surface_definition(&mut e.db, &sid, &leaked, "leak answers", None);
    assert!(
        matches!(err, Err(DbError::Invalid(ref m)) if m.contains("broaden") || m.contains("protected")),
        "{err:?}"
    );
    // Authoritative view must still hide answers after the rejected update.
    assert!(get_view(&mut e.db, &sid)
        .unwrap()
        .state
        .get("answers")
        .is_none());
    assert_eq!(raw_state(&e.db, &sid).0["answers"], json!([1, 0, 0]));
}

#[test]
fn ai_turn_context_returns_public_state_without_quiz_answers() {
    let mut e = env();
    let sid = create(&mut e, fixture_json("quiz"));
    let _ = get_view(&mut e.db, &sid).unwrap();
    let _ = act(&mut e, &sid, "a-1", "answer", json!({"choice": 1}));

    let ctx = ai_turn_context(&mut e.db, &sid).unwrap();
    assert_eq!(ctx["authority"], "rust");
    assert!(ctx["publicState"].get("answers").is_none());
    assert_eq!(ctx["publicState"]["score"], 1);
    assert!(ctx["modelDefinition"]["stateSchema"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["key"] == "answers"
            && c["initialValue"].is_null()
            && c["valueRedacted"] == true));
    let catalog = &ctx["capabilityCatalog"];
    assert!(catalog["publicState"].get("answers").is_none());
    let visible = catalog["visibleStateKeys"].as_array().unwrap();
    assert!(!visible.iter().any(|k| k == "answers"));
    let encoded = ctx.to_string();
    assert!(
        !encoded.contains("[1,0,0]") && !encoded.contains("[1, 0, 0]"),
        "raw answers array must not appear in AI turn context: {encoded}"
    );
}

#[test]
fn history_redacts_drawn_card_string_params() {
    let mut e = env();
    let sid = create(&mut e, fixture_json("quiz"));
    let view = get_view(&mut e.db, &sid).unwrap();
    let next_seq = view.seq + 1;
    let now = crate::db::now_rfc3339();
    e.db
        .conn()
        .execute(
            "INSERT INTO interactive_action_log (id, surface_id, seq, event_id, action_id, actor, origin,
                params_json, definition_revision, pre_state_revision, post_state_revision, post_state_hash,
                snapshot_json, created_at)
             VALUES ('ial-drawn', ?1, ?2, 'draw-1', 'draw', 'player', 'user', ?3, 1, ?4, ?5, 'hash', NULL, ?6)",
            rusqlite::params![
                sid,
                next_seq,
                json!({"card": "QH", "note": "secret-long-value"}).to_string(),
                view.state_revision,
                view.state_revision + 1,
                now,
            ],
        )
        .unwrap();

    let entries = history(&e.db, &sid).unwrap();
    let drawn = entries
        .iter()
        .find(|h| h.event_id == "draw-1")
        .expect("drawn card history row");
    assert_eq!(drawn.params["card"]["redacted"], true);
    assert_eq!(drawn.params["note"]["redacted"], true);
    let encoded = serde_json::to_string(&drawn.params).unwrap();
    assert!(!encoded.contains("QH"));
    assert!(!encoded.contains("secret-long-value"));
}

#[test]
fn quiz_projections_never_expose_answers_in_definitions_or_history() {
    let mut e = env();
    let sid = create(&mut e, fixture_json("quiz"));
    let view = get_view(&mut e.db, &sid).unwrap();
    assert!(view.state.get("answers").is_none());

    let surface = get_surface(&e.db, &sid).unwrap();
    let def: InteractiveAppDefinition =
        serde_json::from_value(surface.definition["interactive"].clone()).unwrap();
    for projected in [def.renderer_view(), def.model_view()] {
        let answers = projected["stateSchema"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["key"] == "answers")
            .unwrap();
        assert!(answers["initialValue"].is_null());
        assert_eq!(answers["valueRedacted"], true);
        assert!(projected.get("randomSeed").is_none());
    }
    assert!(def.renderer_view().get("testCases").is_none());

    let _ = act(&mut e, &sid, "a-1", "answer", json!({"choice": 1}));
    for entry in history(&e.db, &sid).unwrap() {
        let encoded = serde_json::to_string(&entry).unwrap();
        assert!(
            !encoded.contains("\"answers\""),
            "history must not carry answers key: {encoded}"
        );
    }
    for seq in 1..=get_view(&mut e.db, &sid).unwrap().seq {
        assert!(replay_view(&e.db, &sid, seq)
            .unwrap()
            .get("answers")
            .is_none());
    }
}

#[test]
fn history_and_ai_context_never_expose_quiz_answers() {
    let mut e = env();
    let sid = create(&mut e, fixture_json("quiz"));
    let _ = get_view(&mut e.db, &sid).unwrap();
    let _ = act(&mut e, &sid, "a-1", "answer", json!({"choice": 1}));

    let hist = history(&e.db, &sid).unwrap();
    let blob = serde_json::to_string(&hist).unwrap();
    assert!(
        !blob.contains("[1,0,0]") && !blob.contains("[1, 0, 0]"),
        "history must not leak answer array: {blob}"
    );
    // Non-checkpoint action params must be redacted representations.
    let move_entry = hist.iter().find(|h| h.action_id == "answer").unwrap();
    assert!(
        move_entry.params.get("choice").and_then(|v| v.as_i64()) == Some(1)
            || move_entry
                .params
                .get("choice")
                .is_some_and(|v| v.get("redacted").is_some()),
        "choice may stay as number; secrets must not appear as raw strings: {:?}",
        move_entry.params
    );

    let ctx = ai_turn_context(&mut e.db, &sid).unwrap();
    let ctx_s = ctx.to_string();
    assert!(!ctx_s.contains("\"answers\":[1,0,0]") && !ctx_s.contains("\"answers\": [1, 0, 0]"));
    assert_eq!(ctx["authority"], "rust");
    assert!(ctx["publicState"].get("answers").is_none());
    assert!(ctx["modelDefinition"]["stateSchema"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["key"] == "answers" && c["valueRedacted"] == true));
}

#[test]
fn renderer_and_model_definition_projections_redact_restricted_initials() {
    let def = get_fixture("quiz").unwrap();
    let renderer = def.renderer_view();
    let model = def.model_view();
    for projected in [&renderer, &model] {
        let answers = projected["stateSchema"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["key"] == "answers")
            .unwrap();
        assert!(answers["initialValue"].is_null());
        assert_eq!(answers["valueRedacted"], true);
    }
    assert!(renderer.get("testCases").is_none());
    assert!(renderer.get("randomSeed").is_none());
}
