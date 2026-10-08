//! FreeRDP 3 runs in its own X11/XWayland window. Secrets travel only over an anonymous pipe.
use crate::{
    keepass::{self, KeepassState},
    store,
    tools::valid_host,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::Write,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Emitter, State};
use zeroize::Zeroizing;

const FILE: &str = "rdp.json";
#[derive(Clone, Serialize, Deserialize)]
pub struct RdpProfile {
    pub id: String,
    pub name: String,
    pub group: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub domain: String,
    pub auth: String,
    pub keepass_entry: String,
    pub width: u16,
    pub height: u16,
    pub fullscreen: bool,
    pub multimon: bool,
    pub dynamic_resolution: bool,
    pub clipboard: bool,
    pub audio: bool,
    pub admin: bool,
    pub cert: String,
    pub scale: u16,
}
#[derive(Clone, Serialize)]
pub struct Session {
    id: String,
    profile_id: String,
    name: String,
    pid: u32,
}
#[derive(Default, Clone)]
pub struct RdpState(Arc<Mutex<HashMap<String, (Session, Child)>>>);
fn key(id: &str) -> String {
    format!("rdp:{id}")
}
fn text(s: &str) -> bool {
    s.len() <= 1024 && !s.chars().any(char::is_control)
}
fn validate(p: &RdpProfile) -> Result<(), String> {
    if !store::valid_id(&p.id) || p.name.trim().is_empty() || !text(&p.name) || !text(&p.group) {
        return Err("Некорректное имя или ID профиля".into());
    }
    if !valid_host(&p.host) || p.port == 0 {
        return Err("Некорректный адрес или порт".into());
    }
    if !text(&p.user) || !text(&p.domain) {
        return Err("Некорректный пользователь или домен".into());
    }
    if !["prompt", "password", "keepass"].contains(&p.auth.as_str()) {
        return Err("Неизвестный способ входа".into());
    }
    if p.auth == "keepass" && p.keepass_entry.is_empty() {
        return Err("Выберите запись KeePass".into());
    }
    if !(200..=8192).contains(&p.width)
        || !(200..=8192).contains(&p.height)
        || ![100, 140, 180].contains(&p.scale)
    {
        return Err("Некорректный размер или масштаб".into());
    }
    if !["deny", "tofu", "ignore"].contains(&p.cert.as_str()) {
        return Err("Неизвестная проверка сертификата".into());
    }
    Ok(())
}
#[tauri::command]
pub fn rdp_list() -> Result<Vec<RdpProfile>, String> {
    store::load_json(FILE)
}
#[tauri::command]
pub fn rdp_save(
    profile: RdpProfile,
    secret: Option<String>,
    clear_secret: bool,
) -> Result<(), String> {
    let secret = secret.map(Zeroizing::new);
    validate(&profile)?;
    if let Some(s) = secret.as_ref().filter(|s| !s.is_empty()) {
        if s.len() > 4096 || s.chars().any(char::is_control) {
            return Err("Пароль содержит управляющие символы или слишком длинный".into());
        }
        if profile.auth != "password" {
            return Err("Пароль сохраняется только для keyring".into());
        }
        store::secret_set(&key(&profile.id), s)?;
    }
    let mut profiles = rdp_list()?;
    let id = profile.id.clone();
    let remove = clear_secret || profile.auth != "password";
    if let Some(old) = profiles.iter_mut().find(|p| p.id == id) {
        *old = profile;
    } else {
        profiles.push(profile);
    }
    store::save_json(FILE, &profiles)?;
    if remove {
        store::secret_delete(&key(&id));
    }
    Ok(())
}
#[tauri::command]
pub fn rdp_delete(id: String) -> Result<(), String> {
    if !store::valid_id(&id) {
        return Err("Некорректный ID".into());
    }
    let mut profiles = rdp_list()?;
    profiles.retain(|p| p.id != id);
    store::save_json(FILE, &profiles)?;
    store::secret_delete(&key(&id));
    Ok(())
}
#[tauri::command]
pub fn rdp_group_rename(from: String, to: String) -> Result<(), String> {
    if !text(&to) {
        return Err("Некорректное имя группы".into());
    }
    let mut profiles = rdp_list()?;
    for p in profiles.iter_mut().filter(|p| p.group == from) {
        p.group = to.trim().into();
    }
    store::save_json(FILE, &profiles)
}
#[derive(Serialize)]
pub struct Status {
    available: bool,
    version: String,
}
#[tauri::command]
pub async fn rdp_status() -> Result<Status, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let version = Command::new("xfreerdp3")
            .arg("/version")
            .stdin(Stdio::null())
            .output();
        match version {
            Ok(v) => {
                let version = String::from_utf8_lossy(&v.stdout).trim().to_string();
                let help = Command::new("xfreerdp3")
                    .arg("/help")
                    .stdin(Stdio::null())
                    .output()
                    .map_err(store::err)?;
                Ok(Status {
                    available: v.status.success()
                        && version.contains("version 3.")
                        && String::from_utf8_lossy(&help.stdout).contains("/args-from:"),
                    version,
                })
            }
            Err(_) => Ok(Status {
                available: false,
                version: "Установите FreeRDP 3: sudo pacman -S freerdp (X11 / XWayland)".into(),
            }),
        }
    })
    .await
    .map_err(store::err)?
}
fn arguments(p: &RdpProfile, user: &str) -> Vec<String> {
    let host = if p.host.contains(':') {
        format!("[{}]", p.host)
    } else {
        p.host.clone()
    };
    let mut args = vec![
        format!("/v:{host}:{}", p.port),
        format!("/cert:{}", p.cert),
        format!("/size:{}x{}", p.width, p.height),
        format!("/scale:{}", p.scale),
        "/log-level:OFF".into(),
    ];
    if !user.is_empty() {
        args.push(format!("/u:{user}"));
    }
    if !p.domain.is_empty() {
        args.push(format!("/d:{}", p.domain));
    }
    for (yes, arg) in [
        (p.fullscreen, "/f"),
        (p.multimon, "/multimon"),
        (p.dynamic_resolution, "+dynamic-resolution"),
        (p.audio, "/sound"),
        (p.admin, "/admin"),
    ] {
        if yes {
            args.push(arg.into());
        }
    }
    args.push(
        if p.clipboard {
            "+clipboard"
        } else {
            "-clipboard"
        }
        .into(),
    );
    args
}
fn payload(p: &RdpProfile, user: &str, password: &str) -> Result<Zeroizing<String>, String> {
    if !text(user) || password.len() > 4096 || password.chars().any(char::is_control) {
        return Err("Учётные данные содержат управляющие символы или слишком длинные".into());
    }
    let mut input = Zeroizing::new(arguments(p, user).join("\n"));
    if !password.is_empty() {
        input.push_str("\n/p:");
        input.push_str(password);
    }
    input.push('\n');
    Ok(input)
}
#[tauri::command]
pub fn rdp_connect(
    app: AppHandle,
    kp: State<KeepassState>,
    state: State<RdpState>,
    id: String,
    password: Option<String>,
) -> Result<Session, String> {
    let supplied = password.map(Zeroizing::new);
    let p = rdp_list()?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or("Профиль не найден")?;
    validate(&p)?;
    let (user, pass) = match p.auth.as_str() {
        "keepass" => {
            let (u, s) = keepass::credentials(&kp, &p.keepass_entry)?;
            (
                if p.user.is_empty() { u } else { p.user.clone() },
                Zeroizing::new(s),
            )
        }
        "password" => (
            p.user.clone(),
            Zeroizing::new(
                store::secret_get(&key(&p.id))
                    .ok_or("Пароль отсутствует или keyring заблокирован")?,
            ),
        ),
        _ => (
            p.user.clone(),
            supplied.ok_or("Введите пароль для подключения")?,
        ),
    };
    if user.is_empty() || pass.is_empty() {
        return Err("Укажите пользователя и непустой пароль".into());
    }
    let input = payload(&p, &user, &pass)?;
    let mut sessions = state.0.lock().map_err(store::err)?;
    if sessions.len() >= 32 {
        return Err("Слишком много RDP-сессий".into());
    }
    let mut child = Command::new("xfreerdp3")
        .arg("/args-from:stdin")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            "Не удалось запустить xfreerdp3. Установите freerdp и включите X11/XWayland".to_string()
        })?;
    // No shell, environment secrets, temporary files, clipboard or child log capture.
    let written = child
        .stdin
        .take()
        .ok_or("stdin недоступен")
        .and_then(|mut pipe| {
            pipe.write_all(input.as_bytes())
                .map_err(|_| "Ошибка передачи параметров FreeRDP")
        });
    if let Err(e) = written {
        let _ = child.kill();
        let _ = child.wait();
        return Err(e.into());
    }
    let session = Session {
        id: uuid::Uuid::new_v4().to_string(),
        profile_id: p.id,
        name: p.name,
        pid: child.id(),
    };
    sessions.insert(session.id.clone(), (session.clone(), child));
    drop(sessions);
    let shared = state.0.clone();
    let sid = session.id.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(500));
        let Ok(mut list) = shared.lock() else { break };
        let Some((_, child)) = list.get_mut(&sid) else {
            break;
        };
        match child.try_wait() {
            Ok(Some(status)) => {
                list.remove(&sid);
                drop(list);
                let _ = app.emit("rdp-exit", serde_json::json!({"id": sid, "success": status.success(), "code": status.code()}));
                break;
            }
            Ok(None) => {}
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                list.remove(&sid);
                break;
            }
        }
    });
    Ok(session)
}
#[tauri::command]
pub fn rdp_sessions(state: State<RdpState>) -> Result<Vec<Session>, String> {
    Ok(state
        .0
        .lock()
        .map_err(store::err)?
        .values()
        .map(|(s, _)| s.clone())
        .collect())
}
#[tauri::command]
pub fn rdp_disconnect(state: State<RdpState>, id: String) -> Result<(), String> {
    let mut list = state.0.lock().map_err(store::err)?;
    if let Some((_, child)) = list.get_mut(&id) {
        child.kill().map_err(store::err)?;
        child.wait().map_err(store::err)?;
        list.remove(&id);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn profile() -> RdpProfile {
        serde_json::from_value(serde_json::json!({"id":"test-1","name":"server","group":"prod","host":"::1","port":3389,"user":"alice","domain":"","auth":"prompt","keepass_entry":"","width":1280,"height":720,"fullscreen":false,"multimon":false,"dynamic_resolution":true,"clipboard":false,"audio":false,"admin":false,"cert":"deny","scale":100})).unwrap()
    }
    #[test]
    fn safe_arguments() {
        let p = profile();
        validate(&p).unwrap();
        let a = arguments(&p, "DOMAIN\\user");
        assert!(a.contains(&"/v:[::1]:3389".into()));
        assert!(a.contains(&"-clipboard".into()));
        assert!(a.contains(&"/cert:deny".into()));
        assert!(!a.iter().any(|s| s.starts_with("/p:")));
    }
    #[test]
    fn pipe_rejects_injection() {
        let p = profile();
        for bad in ["pw\n/cert:ignore", "pw\r/u:root", "pw\0"] {
            assert!(payload(&p, "alice", bad).is_err());
        }
        assert!(payload(&p, "bob\n/admin", "pw").is_err());
        let s = payload(&p, "alice", "spaces : $'\"\\ password").unwrap();
        assert!(s.ends_with("\n/p:spaces : $'\"\\ password\n"));
    }
    #[test]
    fn validates_loaded_profiles() {
        let mut p = profile();
        p.cert = "ignore\n/admin".into();
        assert!(validate(&p).is_err());
        p = profile();
        p.host = "host /p:secret".into();
        assert!(validate(&p).is_err());
        p = profile();
        p.auth = "bad".into();
        assert!(validate(&p).is_err());
        p = profile();
        p.port = 0;
        assert!(validate(&p).is_err());
    }
}
