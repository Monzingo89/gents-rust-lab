//! Harness compatibility facade over the shared Rust MCP governance core.
pub use gents_local_mcp::governance::*;
#[cfg(test)]
use {rusqlite::Connection, serde_json::Value};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn policy_notifications_are_tenant_run_scoped_and_never_downgrade() {
        let run = uuid::Uuid::new_v4();
        let mut receiver = subscribe("one", run, 1).unwrap();
        publish("two", run, 5).unwrap();
        assert!(!receiver.has_changed().unwrap());
        publish("one", uuid::Uuid::new_v4(), 5).unwrap();
        assert!(!receiver.has_changed().unwrap());
        publish("one", run, 2).unwrap();
        receiver.changed().await.unwrap();
        assert_eq!(*receiver.borrow_and_update(), 2);
        publish("one", run, 1).unwrap();
        assert!(!receiver.has_changed().unwrap());
        let later = subscribe("one", run, 1).unwrap();
        assert_eq!(*later.borrow(), 2);
    }

    #[tokio::test]
    #[ignore = "opt-in real local master delegation and revocation probe"]
    async fn local_master_assigns_and_revokes_capabilities() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let company = format!("governance-probe-{}", uuid::Uuid::new_v4().simple());
        let run = uuid::Uuid::new_v4();
        let owner = "local-evaluation";
        crate::company::operation(
            &root,
            &company,
            owner,
            &json!({"op":"register","name":"Isolated local master evaluation"}),
        )
        .unwrap();
        let ceiling = json!({"tools":["files:read"],"skills":["finance-reconciliation"],"memory_from":["Master"]});
        let empty = json!({"tools":[],"skills":[],"memory_from":[]});
        let initial = json!({"revision":1,"master":"Master",
            "ceilings":{"Master":{"tools":["access_delegate"],"skills":[],"memory_from":[]},"Inspector":ceiling},
            "grants":{"Master":{"tools":["access_delegate"],"skills":[],"memory_from":[]},"Inspector":empty}});
        crate::company::operation(
            &root,
            &company,
            owner,
            &json!({"op":"governance_install","run_id":run,"expected_revision":0,"policy":initial}),
        )
        .unwrap();
        let runtime = crate::local_runtime::LocalRuntime::new(root.clone()).unwrap();
        let model = crate::provider::Model {
            provider: "local".into(),
            model: "Qwen3.5-4B local master".into(),
            path: Some("models/qwen3.5-4b-router/Qwen3.5-4B-Q4_K_M.gguf".into()),
            url: None,
            key_env: None,
            adapter: None,
            context_tokens: Some(8192),
            local_reasoning_tokens: None,
            local_final_tokens: None,
        };
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let mut receipts = Vec::new();
        for (revision, grant) in [(1, ceiling), (2, empty)] {
            let input = json!({"envelope":{"policy_revision":revision},"role_capabilities":[{"id":"access_delegate"}],
                "task_input":{"task":"As runtime master Master, replace Inspector's grant with EXACTLY requested_grant. Propose one access_delegate call, not a plan or question.",
                    "expected_revision":revision,"recipient":"Inspector","requested_grant":grant}});
            let reply=tokio::time::timeout(std::time::Duration::from_secs(180),runtime.complete_scoped(&model,
                "COMPANY_OPERATOR: Return the assessment/proposal envelope. Select exactly one access_delegate call matching the explicit requested recipient, revision and grant. You cannot change ceilings or claim execution.",
                &input.to_string(),512,&cancel,"local-governance-probe")).await.unwrap().unwrap();
            let value: Value = serde_json::from_str(&reply.text).unwrap();
            let proposal = crate::contracts::unassessed_proposal(&value);
            assert_eq!(proposal["action"], "access_delegate");
            let args = crate::contracts::arguments(proposal).unwrap();
            assert_eq!(args["recipient"], "Inspector");
            assert_eq!(args["expected_revision"], revision);
            assert_eq!(args["grant"], grant);
            let receipt =
                crate::company::delegate_access(&root, &company, owner, run, "Master", &args)
                    .unwrap();
            let policy = crate::company::governance(&root, &company, owner, run)
                .unwrap()
                .unwrap();
            assert_eq!(
                serde_json::to_value(&policy.grants["Inspector"]).unwrap(),
                grant
            );
            receipts.push(json!({"local_reply":reply,"executed_request":args,"result":receipt}));
        }
        runtime.stop_owned().await;
        let policy = crate::company::governance(&root, &company, owner, run)
            .unwrap()
            .unwrap();
        assert_eq!(policy.revision, 3);
        assert!(policy
            .authorize("Inspector", "files", &json!({"op":"read"}))
            .is_err());
        let output = root.join("data/model-registry/master-delegation-probe.json");
        assert!(!output.is_symlink());
        std::fs::write(&output,serde_json::to_vec_pretty(&json!({"passed":true,"company_id":company,"run_id":run,"final_revision":3,
            "verified":"real local proposals, deterministic ceiling validation, transactional grant and revocation, revoked tool denial","receipts":receipts})).unwrap()).unwrap();
        println!("{}", json!({"passed":true,"evidence":output}));
    }

    fn policy() -> Policy {
        serde_json::from_value(json!({"revision":1,"master":"Master",
            "ceilings":{"Master":{"tools":["access_delegate"],"skills":[],"memory_from":[]},
                "Worker":{"tools":["files:read","finish"],"skills":[],"memory_from":["Master"]}},
            "grants":{"Master":{"tools":["access_delegate"],"skills":[],"memory_from":[]},
                "Worker":{"tools":["files:read"],"skills":[],"memory_from":[]}}}))
        .unwrap()
    }

    #[test]
    fn role_operations_and_unknown_actors_fail_closed() {
        let p = policy();
        p.validate().unwrap();
        assert!(p
            .authorize("Worker", "files", &json!({"op":"read"}))
            .is_ok());
        for (who, tool, args) in [
            ("Worker", "files", json!({"op":"write"})),
            ("Worker", "files", json!({})),
            ("Unknown", "files", json!({"op":"read"})),
            ("Worker", "email_send", json!({})),
        ] {
            assert!(p.authorize(who, tool, &args).is_err());
        }
    }

    #[test]
    fn master_can_attenuate_but_not_escalate_or_replay() {
        let mut p = policy();
        let allowed = p.ceilings["Worker"].clone();
        assert!(p.delegate("Worker", "Worker", allowed.clone(), 1).is_err());
        let mut excessive = allowed.clone();
        excessive.tools.insert("shell".into());
        assert!(p.delegate("Master", "Worker", excessive, 1).is_err());
        p.delegate("Master", "Worker", allowed.clone(), 1).unwrap();
        assert_eq!(p.revision, 2);
        assert!(p.delegate("Master", "Worker", allowed, 1).is_err());
        p.delegate("Master", "Worker", Access::default(), 2)
            .unwrap();
        assert!(p
            .authorize("Worker", "files", &json!({"op":"read"}))
            .is_err());
    }

    #[test]
    fn persistence_rejects_stale_updates_and_bad_catalog_ids() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE run_governance(run_id TEXT PRIMARY KEY,revision INTEGER,manifest TEXT);",
        )
        .unwrap();
        let mut p = policy();
        save(&db, "run", &p, 0).unwrap();
        assert!(save(&db, "run", &p, 0).is_err());
        p.delegate("Master", "Worker", Access::default(), 1)
            .unwrap();
        save(&db, "run", &p, 1).unwrap();
        assert!(save(&db, "run", &p, 1).is_err());
        assert_eq!(load(&db, "run").unwrap().unwrap().revision, 2);
        p.ceilings
            .get_mut("Worker")
            .unwrap()
            .tools
            .insert("invented_tool".into());
        assert!(p.validate().is_err());
    }
}
