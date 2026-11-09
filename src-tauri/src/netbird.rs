//! NetBird's own daemon owns profiles, keys and SSO. OpsDeck stores only optional server URLs.
use crate::{process, store};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, process::Stdio, sync::Arc, time::Duration};
use tauri::{Emitter, EventTarget, State, WebviewWindow};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{Mutex, OwnedMutexGuard};

const FILE: &str = "netbird.json";
#[derive(Clone, Default)]
pub struct NetbirdState(pub Arc<Mutex<()>>);
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Servers {
    pub management_url: String,
    pub admin_url: String,
}
#[derive(Clone, Serialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub active: bool,
    #[serde(flatten)]
    pub servers: Servers,
}
#[derive(Default, Serialize)]
pub struct Status {
    pub connected: bool,
    pub state: String,
    pub management_url: String,
    pub ip: String,
}
#[derive(Serialize)]
pub struct Snapshot {
    pub profiles: Vec<Profile>,
    pub status: Status,
}
pub fn valid_handle(s: &str) -> bool {
    s == "default" || ((8..=64).contains(&s.len()) && !s.starts_with('-') && s.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-'))
}
fn check_url(s: &str) -> Result<(), String> {
    if s.is_empty() { return Ok(()); }
    if s.chars().any(char::is_control) { return Err("Некорректный URL NetBird".into()); }
    let u = reqwest::Url::parse(s).map_err(|_| "Укажите полный URL NetBird (https://…)".to_string())?;
    if !matches!(u.scheme(), "https" | "http") || u.host_str().is_none() || !u.username().is_empty()
        || u.password().is_some() || u.query().is_some() || u.fragment().is_some() || s.len() > 2048
    { return Err("URL NetBird не должен содержать пароль, параметры или фрагмент".into()); }
    Ok(())
}
fn parse_profiles(s: &str) -> Result<Vec<Profile>, String> {
    let mut lines = s.lines();
    if lines.next().map(|l| l.split_whitespace().collect::<Vec<_>>()) != Some(vec!["ID", "NAME", "ACTIVE"]) {
        return Err("NetBird: нужен CLI с поддержкой profile list --show-id".into());
    }
    let profiles: Vec<Profile> = lines.filter(|l| !l.trim().is_empty()).map(|line| {
        let l = line.trim();
        let split = l.find(char::is_whitespace).ok_or("Некорректный список профилей NetBird")?;
        let (id, name) = l.split_at(split);
        if !valid_handle(id) { return Err("Некорректный ID NetBird".into()); }
        let active = name.trim_end().ends_with("  ✓");
        let name = if active { name.trim_end().trim_end_matches('✓') } else { name }.trim();
        Ok(Profile { id: id.into(), name: name.into(), active, servers: Servers::default() })
    }).collect::<Result<_, String>>()?;
    if profiles.iter().filter(|p| p.active).count() > 1 { return Err("Неоднозначный активный профиль NetBird".into()); }
    if profiles.iter().map(|p| &p.id).collect::<std::collections::BTreeSet<_>>().len() != profiles.len() {
        return Err("Неоднозначные ID NetBird. Выберите профиль в приложении NetBird".into());
    }
    Ok(profiles)
}
fn parse_status(s: &str) -> Result<Status, String> {
    let v: Value = serde_json::from_str(s).map_err(|_| "NetBird: некорректный статус".to_string())?;
    let text = |key: &str| v.get(key).and_then(Value::as_str).unwrap_or("").to_string();
    let ip = text("netbirdIp");
    Ok(Status {
        connected: v["management"]["connected"].as_bool() == Some(true) && v["daemonStatus"] == "Connected" && !ip.is_empty(),
        state: text("daemonStatus"),
        management_url: v["management"]["url"].as_str().unwrap_or("").into(),
        ip,
    })
}
// No shell or privilege escalation. Never include raw CLI output in errors or logs.
fn cli_command(bin: &str, args: &[String]) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(bin);
    cmd.args(["--log-file", "console", "--log-level", "error"]).args(args)
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true);
    process::no_console_tokio(&mut cmd);
    cmd
}
async fn cli_with(bin: &str, args: &[String], seconds: u64) -> Result<String, String> {
    let mut cmd = cli_command(bin, args);
    let out = tokio::time::timeout(Duration::from_secs(seconds), cmd.output()).await
        .map_err(|_| "NetBird: время ожидания истекло. Проверьте вход в браузере и статус подключения".to_string())?
        .map_err(|_| "Не удалось запустить NetBird. Установите CLI и проверьте доступ к службе".to_string())?;
    if !out.status.success() { return Err("NetBird: операция не выполнена. Проверьте службу, права доступа и авторизацию в браузере".into()); }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct LoginPrompt {
    url: String,
    code: Option<String>,
}

#[derive(Default)]
struct LoginOutput(bool);
impl LoginOutput {
    fn line(&mut self, line: &str) -> Option<LoginPrompt> {
        let line = line.trim();
        if matches!(line, "If your browser didn't open automatically, use this URL to log in:" | "Use this URL to log in:") {
            self.0 = true;
            return None;
        }
        if line.is_empty() || !std::mem::take(&mut self.0) || line.len() > 8192 { return None; }
        let (url, tail) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        if url.chars().any(char::is_control) { return None; }
        let url = reqwest::Url::parse(url).ok()?;
        let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if !(url.scheme() == "https" || (local && url.scheme() == "http"))
            || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() { return None; }
        let code = tail.trim().strip_prefix("and enter the code ")
            .and_then(|s| s.strip_suffix(" to authenticate."))
            .filter(|s| !s.is_empty() && s.len() <= 64 && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-'))
            .map(str::to_string);
        Some(LoginPrompt { url: url.into(), code })
    }
}

// Read stdout during `up`: waiting for process exit first would hide the link until login times out.
async fn up_with_login(bin: &str, args: &[String], notify: impl Fn(Option<LoginPrompt>)) -> Result<(), String> {
    let mut child = cli_command(bin, args).spawn()
        .map_err(|_| "Не удалось запустить NetBird. Установите CLI и проверьте доступ к службе".to_string())?;
    let mut lines = BufReader::new(child.stdout.take().expect("stdout is piped")).lines();
    let mut shown = false;
    let result = tokio::time::timeout(Duration::from_secs(180), async {
        let mut parser = LoginOutput::default();
        while let Some(line) = lines.next_line().await? {
            if let Some(prompt) = parser.line(&line) { shown = true; notify(Some(prompt)); }
        }
        child.wait().await
    }).await;
    if shown { notify(None); }
    match result {
        Ok(Ok(status)) if status.success() => Ok(()),
        Err(_) => Err("NetBird: время ожидания истекло. Проверьте вход в браузере и статус подключения".into()),
        _ => Err("NetBird: операция не выполнена. Проверьте службу, права доступа и авторизацию в браузере".into()),
    }
}

async fn cli(args: &[&str], seconds: u64) -> Result<String, String> {
    cli_with("netbird", &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(), seconds).await
}
async fn native_profiles_with(bin: &str) -> Result<Vec<Profile>, String> {
    parse_profiles(&cli_with(bin, &["profile".into(), "list".into(), "--show-id".into()], 10).await?)
}
async fn native_profiles() -> Result<Vec<Profile>, String> { native_profiles_with("netbird").await }
async fn status_with(bin: &str) -> Result<Status, String> {
    parse_status(&cli_with(bin, &["status".into(), "--json".into()], 10).await?)
}
async fn status() -> Result<Status, String> { status_with("netbird").await }
#[tauri::command]
pub async fn netbird_snapshot() -> Result<Snapshot, String> {
    let mut profiles = native_profiles().await?;
    let saved: BTreeMap<String, Servers> = store::load_json(FILE)?;
    for p in &mut profiles { p.servers = saved.get(&p.id).cloned().unwrap_or_default(); }
    Ok(Snapshot { profiles, status: status().await? })
}
#[tauri::command]
pub async fn netbird_save(state: State<'_, NetbirdState>, id: Option<String>, name: String, servers: Servers) -> Result<(), String> {
    let _guard = state.0.lock().await;
    let name = name.trim();
    if name.is_empty() || name.len() > 128 || name.starts_with('-') || name.chars().any(char::is_control) {
        return Err("Укажите имя профиля NetBird без управляющих символов".into());
    }
    check_url(&servers.management_url)?; check_url(&servers.admin_url)?;
    let mut saved: BTreeMap<String, Servers> = store::load_json(FILE)?;
    let profiles = native_profiles().await?;
    let id = if let Some(id) = id {
        let p = profiles.iter().find(|p| p.id == id).ok_or("Профиль NetBird не найден")?;
        if p.name != name { cli(&["profile", "rename", &id, name], 15).await?; }
        id
    } else {
        let out = cli(&["profile", "add", name], 15).await?;
        out.lines().find_map(|l| l.strip_prefix("Profile added: ").and_then(|s| s.split_whitespace().next()))
            .filter(|s| valid_handle(s)).ok_or("NetBird создал профиль, но не вернул ID. Обновите список")?.to_string()
    };
    saved.insert(id, servers);
    store::save_json(FILE, &saved)
}
#[tauri::command]
pub async fn netbird_remove(state: State<'_, NetbirdState>, id: String) -> Result<(), String> {
    let _guard = state.0.lock().await;
    let p = native_profiles().await?.into_iter().find(|p| p.id == id).ok_or("Профиль NetBird не найден")?;
    if p.active || id == "default" { return Err("Активный профиль и default нельзя удалить".into()); }
    cli(&["profile", "remove", &id], 15).await?;
    let mut saved: BTreeMap<String, Servers> = store::load_json(FILE)?;
    saved.remove(&id); store::save_json(FILE, &saved)
}
fn up_args(id: &str, servers: &Servers) -> Result<Vec<String>, String> {
    if !valid_handle(id) { return Err("Некорректный ID NetBird".into()); }
    check_url(&servers.management_url)?; check_url(&servers.admin_url)?;
    let mut args = vec!["up".into(), "--profile".into(), id.into()];
    for (flag, value) in [("--management-url", &servers.management_url), ("--admin-url", &servers.admin_url)] {
        if !value.is_empty() { args.extend([flag.into(), value.clone()]); }
    }
    Ok(args)
}
fn server_matches(expected: &str, actual: &str) -> bool {
    if expected.is_empty() { return true; }
    match (reqwest::Url::parse(expected), reqwest::Url::parse(actual)) {
        (Ok(a), Ok(b)) => a.scheme() == b.scheme() && a.host_str() == b.host_str()
            && a.port_or_known_default() == b.port_or_known_default()
            && a.path().trim_end_matches('/') == b.path().trim_end_matches('/'),
        _ => false,
    }
}
/// Hold this guard through FreeRDP spawn, so another OpsDeck operation cannot switch the VPN in between.
pub async fn prepare(state: &NetbirdState, id: &str, window: &WebviewWindow) -> Result<OwnedMutexGuard<()>, String> {
    let guard = state.0.clone().lock_owned().await;
    let servers: BTreeMap<String, Servers> = store::load_json(FILE)?;
    connect_using("netbird", id, &servers.get(id).cloned().unwrap_or_default(), |prompt| {
        let _ = window.emit_to(EventTarget::webview_window(window.label()), "netbird-login", prompt);
    }).await?;
    Ok(guard)
}
async fn connect_using(bin: &str, id: &str, servers: &Servers, notify: impl Fn(Option<LoginPrompt>)) -> Result<(), String> {
    let profiles = native_profiles_with(bin).await?;
    let p = profiles.iter().find(|p| p.id == id).ok_or("Привязанный профиль NetBird не найден")?;
    let current = status_with(bin).await?;
    // Explicit server edits must be applied on the next connection, even for the active profile.
    if !(p.active && current.connected && servers.admin_url.is_empty() && server_matches(&servers.management_url, &current.management_url)) {
        up_with_login(bin, &up_args(id, servers)?, notify).await?;
    }
    for _ in 0..20 {
        let ready = status_with(bin).await?;
        if ready.connected && server_matches(&servers.management_url, &ready.management_url)
            && native_profiles_with(bin).await?.iter().any(|p| p.active && p.id == id) { return Ok(()); }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Err("NetBird не подтвердил подключение нужного профиля. RDP не запущен".into())
}
#[tauri::command]
pub async fn netbird_connect(window: WebviewWindow, state: State<'_, NetbirdState>, id: String) -> Result<(), String> {
    let _guard = prepare(&state, &id, &window).await?;
    Ok(())
}
#[tauri::command]
pub async fn netbird_disconnect(state: State<'_, NetbirdState>) -> Result<(), String> {
    let _guard = state.0.lock().await;
    cli(&["down"], 20).await?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_ids_and_names_with_spaces_or_duplicates() {
        let p = parse_profiles("ID        NAME              ACTIVE\ndefault   default\n1234abcd  Work Team  1   ✓\n5678efab  Work Team  1\n").unwrap();
        assert_eq!(p[1].name, "Work Team  1"); assert!(p[1].active);
        assert_ne!(p[1].id, p[2].id);
        assert!(parse_profiles("NAME ACTIVE\nwork ✓").is_err());
        assert!(parse_profiles("ID NAME ACTIVE\n--flag work").is_err());
        assert!(parse_profiles("ID NAME ACTIVE\n1234abcd one\n1234abcd two").is_err());
    }
    #[test]
    fn urls_and_argv_cannot_inject_options_or_credentials() {
        for bad in ["file:///etc/passwd", "https://u:password@host", "https://host/?token=secret", "https://host/#secret"] { assert!(check_url(bad).is_err()); }
        assert!(up_args("--setup-key", &Servers::default()).is_err());
        assert!(up_args("--deadbeef", &Servers::default()).is_err());
        assert_eq!(up_args("1234abcd", &Servers::default()).unwrap(), vec!["up", "--profile", "1234abcd"]);
        assert!(up_args("default", &Servers { management_url: "https://vpn.example.com:443".into(), admin_url: String::new() }).unwrap().contains(&"https://vpn.example.com:443".into()));
        assert!(server_matches("https://vpn.example.com", "https://vpn.example.com:443/"));
        assert!(!server_matches("https://vpn.example.com", "https://other.example.com"));
    }
    #[test]
    fn connected_requires_daemon_management_and_ip() {
        assert!(!parse_status(r#"{"daemonStatus":"NeedsLogin","management":{"connected":false}}"#).unwrap().connected);
        assert!(!parse_status(r#"{"daemonStatus":"Connected","management":{"connected":true}}"#).unwrap().connected);
        assert!(parse_status(r#"{"daemonStatus":"Connected","management":{"connected":true},"netbirdIp":"100.64.1.2/16"}"#).unwrap().connected);
    }
    #[test]
    fn login_parser_only_accepts_announced_browser_links() {
        let mut p = LoginOutput::default();
        assert!(p.line("https://auth.example.com/unrelated").is_none());
        for invalid in ["javascript:alert(1)", "https://user:pass@auth.example.com/", "http://auth.example.com/"] {
            assert!(p.line("Use this URL to log in:").is_none());
            assert!(p.line(invalid).is_none());
        }
        p.line("If your browser didn't open automatically, use this URL to log in:");
        p.line("");
        assert_eq!(p.line("https://auth.example.com/device?session=fake and enter the code ABCD-EFGH to authenticate."), Some(LoginPrompt {
            url: "https://auth.example.com/device?session=fake".into(), code: Some("ABCD-EFGH".into()),
        }));
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn browser_link_arrives_before_cli_finishes_and_is_cleared() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("opsdeck-netbird-sso-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let script = dir.join("netbird");
        let seen = dir.join("seen");
        std::fs::write(&script, r#"#!/bin/sh
cd "$(dirname "$0")" || exit 1
printf 'Use this URL to log in:\n\nhttps://auth.example.com/device?session=fake\n'
for i in 1 2 3 4 5 6 7 8 9 10; do
  if [ -f seen ]; then touch acknowledged; exit 7; fi
  sleep 0.1
done
exit 8
"#).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        let prompts = std::cell::RefCell::new(Vec::new());
        let result = up_with_login(script.to_str().unwrap(), &[], |prompt| {
            if prompt.is_some() { std::fs::write(&seen, "").unwrap(); }
            prompts.borrow_mut().push(prompt);
        }).await;
        assert!(result.unwrap_err().contains("операция не выполнена"));
        assert!(dir.join("acknowledged").exists());
        assert_eq!(prompts.into_inner(), vec![Some(LoginPrompt { url: "https://auth.example.com/device?session=fake".into(), code: None }), None]);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn cli_connection_gate_reuses_login_and_fails_closed() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("opsdeck-netbird-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let script = dir.join("netbird");
        std::fs::write(&script, r#"#!/bin/sh
cd "$(dirname "$0")" || exit 1
shift 4
case "$1" in
  profile)
    printf 'ID        NAME          ACTIVE\n'
    if [ -f selected ]; then printf '1234abcd  Office  VPN   ✓\ndefault   default\n';
    else printf 'default   default       ✓\n1234abcd  Office  VPN\n'; fi ;;
  status)
    if [ -f selected ]; then printf '{"daemonStatus":"Connected","management":{"connected":true,"url":"https://vpn.example.com:443"},"netbirdIp":"100.64.1.2"}';
    else printf '{"daemonStatus":"NeedsLogin","management":{"connected":false}}'; fi ;;
  up)
    printf '%s\n' "$@" >> calls
    if [ -f reject ]; then printf 'https://auth.example.com/?token=private'; exit 7; fi
    touch selected ;;
  *) exit 8 ;;
esac
"#).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        let bin = script.to_str().unwrap();
        let servers = Servers { management_url: "https://vpn.example.com".into(), admin_url: String::new() };
        connect_using(bin, "1234abcd", &servers, |_| {}).await.unwrap();
        let calls = std::fs::read_to_string(dir.join("calls")).unwrap();
        assert!(calls.contains("--profile\n1234abcd\n--management-url\nhttps://vpn.example.com"));
        // Already connected: no second up or SSO, and unknown bindings never select another profile.
        connect_using(bin, "1234abcd", &Servers::default(), |_| {}).await.unwrap();
        assert_eq!(calls, std::fs::read_to_string(dir.join("calls")).unwrap());
        assert!(connect_using(bin, "deadbeef", &servers, |_| {}).await.is_err());
        std::fs::remove_file(dir.join("selected")).unwrap();
        std::fs::write(dir.join("reject"), "").unwrap();
        let err = connect_using(bin, "1234abcd", &servers, |_| {}).await.unwrap_err();
        assert!(!err.contains("token") && !err.contains("private"));
        assert!(!dir.join("selected").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
