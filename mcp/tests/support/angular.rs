use gents_local_mcp::angular_check::{
    runtime::HostSdk, BrowserAction, BrowserCase, BrowserContract, Contract,
};
use std::{collections::BTreeMap, path::PathBuf};

pub fn installed() -> (Contract, HostSdk, BTreeMap<String, String>) {
    let get = |name| {
        PathBuf::from(
            std::env::var_os(name)
                .unwrap_or_else(|| panic!("{name} must explicitly select an installed SDK")),
        )
    };
    let sdk = HostSdk {
        node: get("GENTS_NODE"),
        angular: get("GENTS_ANGULAR_MODULES"),
        driver: get("GENTS_PLAYWRIGHT_DRIVER"),
        browser: get("GENTS_ANGULAR_BROWSER"),
    }
    .canonical()
    .unwrap();
    let version = std::env::var("GENTS_NODE_VERSION").expect("GENTS_NODE_VERSION required");
    let fixture = get("GENTS_ANGULAR_FIXTURE");
    let files = [
        "angular.json",
        "package.json",
        "package-lock.json",
        "tsconfig.json",
        "tsconfig.app.json",
        "src/main.ts",
        "src/index.html",
    ]
    .into_iter()
    .map(|path| {
        (
            path.to_string(),
            std::fs::read_to_string(fixture.join(path)).unwrap(),
        )
    })
    .collect::<BTreeMap<_, _>>();
    let frozen_inputs = files
        .iter()
        .filter(|(p, _)| p.ends_with(".json"))
        .map(|(p, s)| (p.clone(), gents_local_mcp::digest(s.as_bytes())))
        .collect();
    let contract = Contract {
        schema_version: 1,
        id: "angular-fixture".into(),
        actor: "owner-test".into(),
        project_path: "angular.json".into(),
        project_name: "fixture".into(),
        source_paths: files
            .keys()
            .filter(|p| p.as_str() != "angular.json")
            .cloned()
            .collect(),
        frozen_inputs,
        sdk: sdk.pins(&version).unwrap(),
        browser: BrowserContract {
            schema_version: 1,
            startup_timeout_ms: 5000,
            step_timeout_ms: 1000,
            runtime_timeout_ms: 10000,
            cases: vec![
                BrowserCase {
                    id: "initial".into(),
                    action: BrowserAction::ExpectText,
                    selector: "#value".into(),
                    value: Some("0".into()),
                },
                BrowserCase {
                    id: "increment".into(),
                    action: BrowserAction::Click,
                    selector: "#increment".into(),
                    value: None,
                },
                BrowserCase {
                    id: "changed".into(),
                    action: BrowserAction::ExpectText,
                    selector: "#value".into(),
                    value: Some("1".into()),
                },
            ],
        },
    };
    (contract, sdk, files)
}
