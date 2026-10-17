#[cfg(not(target_os = "ios"))]
#[tauri::command]
async fn login(tenant: String, client_id: String, api_id: String) -> Result<String, String> {
    desktop_login(tenant, client_id, api_id)
        .await
        .map_err(|e| e.to_string())
}
#[cfg(not(target_os = "ios"))]
async fn desktop_login(
    tenant: String,
    client_id: String,
    api_id: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use sha2::{Digest, Sha256};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for id in [&tenant, &client_id, &api_id] {
        uuid::Uuid::parse_str(id)?;
    }
    let verifier = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = uuid::Uuid::new_v4().to_string();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let redirect = format!(
        "http://localhost:{}/callback",
        listener.local_addr()?.port()
    );
    let scope = format!("api://{api_id}/Harness.Run");
    let mut authorize = url::Url::parse(&format!(
        "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize"
    ))?;
    authorize.query_pairs_mut().extend_pairs([
        ("client_id", client_id.as_str()),
        ("response_type", "code"),
        ("redirect_uri", redirect.as_str()),
        ("response_mode", "query"),
        ("scope", scope.as_str()),
        ("state", state.as_str()),
        ("code_challenge", challenge.as_str()),
        ("code_challenge_method", "S256"),
    ]);
    open::that(authorize.as_str())?;
    let (mut socket, _) =
        tokio::time::timeout(std::time::Duration::from_secs(180), listener.accept()).await??;
    let mut buffer = [0u8; 8192];
    let count = tokio::time::timeout(std::time::Duration::from_secs(10), socket.read(&mut buffer))
        .await??;
    let request = std::str::from_utf8(&buffer[..count])?;
    let path = request
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .ok_or("invalid callback")?;
    let callback = url::Url::parse(&format!("http://localhost{path}"))?;
    let params: std::collections::HashMap<_, _> = callback.query_pairs().into_owned().collect();
    if callback.path() != "/callback" || params.get("state") != Some(&state) {
        return Err("callback validation failed; retry sign-in".into());
    }
    socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nReturn to Gents Office.").await?;
    let code = params.get("code").ok_or("sign-in cancelled or denied")?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let response = client
        .post(format!(
            "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token"
        ))
        .form(&[
            ("client_id", client_id.as_str()),
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("code_verifier", verifier.as_str()),
            ("scope", scope.as_str()),
        ])
        .send()
        .await?;
    if !response.status().is_success() {
        return Err("token exchange rejected; verify app registration".into());
    }
    let v: serde_json::Value = response.json().await?;
    Ok(v["access_token"].as_str().ok_or("no access token")?.into())
}
#[cfg(target_os = "ios")]
#[tauri::command]
async fn login(_tenant: String, _client_id: String, _api_id: String) -> Result<String, String> {
    Err("iOS native MSAL bridge is an exercise and must be implemented before release. See docs/09-apple-release.md".into())
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![login])
        .run(tauri::generate_context!())
        .expect("Tauri failed");
}
