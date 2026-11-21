mod support;
#[cfg(target_os = "macos")]
use gents_local_mcp::{authority::Authority, governance};
use gents_local_mcp::{dotnet_check, Registry};
use serde_json::{json, Value};
use support::Fixture;
const PROJECT:&str="<Project Sdk=\"Microsoft.NET.Sdk.Web\"><PropertyGroup><TargetFramework>net10.0</TargetFramework><UseAppHost>false</UseAppHost><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>";
#[cfg(target_os = "macos")]
const GOOD: &str = r#"var builder=WebApplication.CreateSlimBuilder(args);
builder.WebHost.ConfigureKestrel(o=>o.ListenHandle(ulong.Parse(Environment.GetEnvironmentVariable("GENTS_API_LISTEN_FD")!)));
var app=builder.Build();app.MapGet("/health",()=>new{ok=true});
app.MapPost("/sum",(Operands p)=>new{sum=p.left+p.right});app.Run();
record Operands(int left,int right);"#;
fn intake() -> Value {
    json!({"dotnet_check_contracts":[{"id":"http-cases","actor":"Worker","project_path":"api/Api.csproj","source_paths":["api/Program.cs"],"sdk_version":"10.0.401","dotnet_sha256":"a".repeat(64),"artifact_mode":"managed_dll","frozen_inputs":{"api/Api.csproj":gents_local_mcp::digest(PROJECT.as_bytes())},"api":{"schema_version":1,"listen_mode":"inherited_loopback_socket","startup_timeout_ms":3000,"request_timeout_ms":1000,"runtime_timeout_ms":8000,"cases":[{"id":"health","method":"GET","path":"/health","expected_status":200,"expected_json":{"ok":true}},{"id":"sum","method":"POST","path":"/sum","request_json":{"left":7,"right":5},"expected_status":200,"expected_json":{"sum":12}}]}}]})
}
#[test]
fn api_contract_is_explicit_and_does_not_grant_candidate_execution() {
    dotnet_check::validate(&intake()).unwrap();
    let f = Fixture::new();
    let registry = Registry::new();
    assert!(registry
        .call(
            &f.authority,
            &f.scope,
            "dotnet_api_check",
            json!({"contract_id":"http-cases"})
        )
        .is_err());
    assert!(registry.list(&f.authority, &f.scope).unwrap()["tools"]
        .as_array()
        .unwrap()
        .iter()
        .all(|t| t["name"] != "dotnet_api_check"));
    for (key, value) in [
        ("listen_mode", json!("any-interface")),
        ("runtime_timeout_ms", json!(300000)),
        ("endpoint", json!("http://example.com")),
    ] {
        let mut bad = intake();
        bad["dotnet_check_contracts"][0]["api"][key] = value;
        assert!(dotnet_check::validate(&bad).is_err());
    }
}
#[cfg(target_os = "macos")]
#[tokio::test]
#[ignore = "actual application execution: requires owner-configured GENTS_DOTNET and installed SDK 10.0.401"]
async fn real_host_http_oracle_separates_compile_api_failure_and_self_reported_success() {
    let executable = dotnet_check::executable_path().unwrap();
    let f = Fixture::new();
    support::install_files(&f);
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    policy.revision = 3;
    for access in [&mut policy.ceilings, &mut policy.grants] {
        access
            .get_mut("Worker")
            .unwrap()
            .tools
            .insert("dotnet_check".into());
    }
    governance::save(&db, &f.scope.run.to_string(), &policy, 2).unwrap();
    let mut contract = intake();
    contract["dotnet_check_contracts"][0]["dotnet_sha256"] = json!(gents_local_mcp::digest(
        &std::fs::read(&executable).unwrap()
    ));
    db.execute("UPDATE jobs SET intake=?1", [contract.to_string()])
        .unwrap();
    let workspace = f.authority.workspace(&f.scope).unwrap();
    std::fs::create_dir_all(workspace.join("api")).unwrap();
    std::fs::write(workspace.join("api/Api.csproj"), PROJECT).unwrap();
    std::fs::write(workspace.join("api/Program.cs"), GOOD).unwrap();
    let canary = f.root.join("api-host-canary");
    std::fs::write(&canary, "untouched synthetic owner data").unwrap();
    let blocked = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    blocked.set_nonblocking(true).unwrap();
    let boundary = format!(
        r#"app.MapGet("/boundary",()=>{{
bool read=false,write=false,scratch=false,bind=false,process=false,network=false;
try{{System.IO.File.ReadAllText({canary});}}catch{{read=true;}}
try{{System.IO.File.WriteAllText({canary},"changed");}}catch{{write=true;}}
try{{System.IO.File.WriteAllText("runtime-write","changed");}}catch{{scratch=true;}}
try{{var s=new System.Net.Sockets.TcpListener(System.Net.IPAddress.Any,0);s.Start();s.Stop();}}catch{{bind=true;}}
try{{using var p=System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo({dotnet},"--version"){{UseShellExecute=false}});p!.WaitForExit(1000);}}catch{{process=true;}}
try{{using var s=new System.Net.Sockets.TcpClient();s.Connect("127.0.0.1",{port});}}catch{{network=true;}}
UIntPtr size=(UIntPtr)8192;var buffer=System.Runtime.InteropServices.Marshal.AllocHGlobal(8192);
int result=Native.sysctl(new int[]{{1,49,{parent}}},3,buffer,ref size,IntPtr.Zero,UIntPtr.Zero);
int error=System.Runtime.InteropServices.Marshal.GetLastPInvokeError();System.Runtime.InteropServices.Marshal.FreeHGlobal(buffer);
bool procargs=result!=0 && (error==1 || error==13);
return new{{read,write,scratch,bind,process,network,procargs}};
}});
"#,
        canary = serde_json::to_string(canary.to_str().unwrap()).unwrap(),
        dotnet = serde_json::to_string(executable.to_str().unwrap()).unwrap(),
        port = blocked.local_addr().unwrap().port(),
        parent = std::process::id()
    );
    let native = r#"class Native { [System.Runtime.InteropServices.DllImport("/usr/lib/libSystem.B.dylib",SetLastError=true)] public static extern int sysctl(int[] name,uint length,IntPtr oldp,ref UIntPtr oldlen,IntPtr newp,UIntPtr newlen); }"#;
    let good = GOOD.replace("app.Run();", &(boundary + "app.Run();")) + native;
    std::fs::write(workspace.join("api/Program.cs"), &good).unwrap();
    contract["dotnet_check_contracts"][0]["api"]["cases"].as_array_mut().unwrap().push(json!({"id":"boundaries","method":"GET","path":"/boundary","expected_status":200,"expected_json":{"read":true,"write":true,"scratch":true,"bind":true,"process":true,"network":true,"procargs":true}}));
    db.execute("UPDATE jobs SET intake=?1", [contract.to_string()])
        .unwrap();
    let registry = Registry::new();
    let mut scope = f.scope.clone();
    scope.expected_revision = Some(3);
    assert!(dotnet_check::api_request_identity(&f.authority, &scope, "http-cases").is_err());
    let build = dotnet_check::request_identity(&f.authority, &scope, "http-cases").unwrap();
    assert!(registry
        .call(
            &f.authority,
            &scope,
            "dotnet_api_check",
            json!({"contract_id":"http-cases","expected_source_identity":build})
        )
        .is_err());
    policy.revision = 4;
    for access in [&mut policy.ceilings, &mut policy.grants] {
        access
            .get_mut("Worker")
            .unwrap()
            .tools
            .insert("dotnet_api_check".into());
    }
    governance::save(&db, &scope.run.to_string(), &policy, 3).unwrap();
    scope.expected_revision = Some(4);
    let directory = f.root.join("target/debug");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_gents-local-mcp"),
        directory.join("gents-local-mcp"),
    )
    .unwrap();
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut receipts = vec![];
    for transport in ["stdio", "sse"] {
        std::env::set_var("GENTS_MCP_TRANSPORT", transport);
        let identity =
            dotnet_check::api_request_identity(&f.authority, &scope, "http-cases").unwrap();
        let result = gents_local_mcp::client::invoke(
            &f.root,
            &scope,
            "dotnet_api_check",
            &json!({"contract_id":"http-cases","expected_source_identity":identity}),
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(result["ok"], true, "{result}");
        assert_eq!(result["result"]["status"], "api_passed", "{result}");
        assert_eq!(result["result"]["compile_passed"], true);
        assert_eq!(result["result"]["api"]["http_requests"], 3);
        assert_eq!(result["result"]["api"]["cases"][2]["status"], "passed");
        assert_eq!(result["result"]["api"]["cases"][1]["status"], "passed");
        assert_eq!(result["result"]["qualified"], false);
        let view = dotnet_check::observations(&f.authority, &scope).unwrap();
        assert_eq!(view[0]["latest_current_api"]["status"], "api_passed");
        assert_eq!(
            view[0]["latest_current_api"]["canonical_receipt_verified"],
            true
        );
        assert!(view[0]["latest_current_build"].is_null());
        receipts.push(result);
    }
    std::env::remove_var("GENTS_MCP_TRANSPORT");
    std::fs::write(
        workspace.join("api/Program.cs"),
        good.replace("p.left+p.right", "p.left-p.right"),
    )
    .unwrap();
    assert!(
        dotnet_check::observations(&f.authority, &scope).unwrap()[0]["latest_current_api"]
            .is_null()
    );
    let identity = dotnet_check::api_request_identity(&f.authority, &scope, "http-cases").unwrap();
    let failed = registry
        .call(
            &f.authority,
            &scope,
            "dotnet_api_check",
            json!({"contract_id":"http-cases","expected_source_identity":identity}),
        )
        .unwrap();
    assert_eq!(failed["value"]["compile_passed"], true);
    assert_eq!(failed["value"]["status"], "api_failed");
    assert_eq!(failed["value"]["api"]["cases"][0]["status"], "passed");
    assert_eq!(failed["value"]["api"]["cases"][1]["status"], "failed");
    receipts.push(failed);
    std::fs::write(
        workspace.join("api/Program.cs"),
        "Console.WriteLine(\"{\\\"passed\\\":true}\");",
    )
    .unwrap();
    let identity = dotnet_check::api_request_identity(&f.authority, &scope, "http-cases").unwrap();
    let spoofed = registry
        .call(
            &f.authority,
            &scope,
            "dotnet_api_check",
            json!({"contract_id":"http-cases","expected_source_identity":identity}),
        )
        .unwrap();
    assert_eq!(spoofed["value"]["compile_passed"], true);
    assert_ne!(spoofed["value"]["status"], "api_passed");
    receipts.push(spoofed);
    std::fs::write(
        workspace.join("api/Program.cs"),
        GOOD.replace(
            "()=>new{ok=true}",
            "async ()=>{await Task.Delay(10000);return new{ok=true};}",
        ),
    )
    .unwrap();
    let identity = dotnet_check::api_request_identity(&f.authority, &scope, "http-cases").unwrap();
    let start = std::time::Instant::now();
    let timed_out = registry
        .call(
            &f.authority,
            &scope,
            "dotnet_api_check",
            json!({"contract_id":"http-cases","expected_source_identity":identity}),
        )
        .unwrap();
    assert_eq!(timed_out["value"]["compile_passed"], true);
    assert_eq!(timed_out["value"]["status"], "api_failed");
    assert_eq!(timed_out["value"]["api"]["http_requests"], 1);
    assert_eq!(timed_out["value"]["api"]["cases"][0]["status"], "failed");
    assert_eq!(timed_out["value"]["api"]["cases"][1]["status"], "not_run");
    assert!(
        timed_out["value"]["api"]["runtime"]["elapsed_ms"]
            .as_u64()
            .unwrap()
            < 5000
    );
    assert!(start.elapsed() < std::time::Duration::from_secs(15));
    receipts.push(timed_out);
    assert_eq!(
        std::fs::read_to_string(canary).unwrap(),
        "untouched synthetic owner data"
    );
    assert!(blocked.accept().is_err());
    assert!(!workspace.join("runtime-write").exists());
    if let Some(path) = std::env::var_os("GENTS_API_TEST_REPORT") {
        std::fs::write(path,serde_json::to_vec_pretty(&json!({"schema_version":1,"scope":"Actual native SDK/application/host HTTP checks; no model or specialist qualification","project":PROJECT,"good_source":good,"receipts":receipts,"checks":{"extra_execution_grant_required":true,"actual_stdio_http":true,"actual_sse_http":true,"current_api_observation_and_source_invalidation":true,"compile_success_does_not_pass_bad_api":true,"zero_exit_stdout_cannot_spoof_http":true,"unresponsive_api_deadline_stops_after_one_request":true,"runtime_denies_host_io_writes_binds_outbound_subprocesses_and_procargs":true},"model_calls":0,"provider_cost_usd":0,"qualified":false,"promoted":false})).unwrap()).unwrap();
    }
}
