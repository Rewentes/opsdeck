import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { helpBtn } from "./help";
import { icon } from "./icons";
import { kpEntries, kpStatus, pickEntry } from "./keepass";
import { registerProvider } from "./palette";
import { ask, esc, overlay, toast } from "./ui";

type Profile = {
  id: string; name: string; group: string; host: string; port: number; user: string; domain: string;
  auth: string; keepass_entry: string; width: number; height: number; scale: number; cert: string;
  fullscreen: boolean; multimon: boolean; dynamic_resolution: boolean; clipboard: boolean; audio: boolean; admin: boolean;
};
type Session = { id: string; profile_id: string; name: string; pid: number };
type Status = { available: boolean; version: string };
const AUTH: Record<string, string> = { prompt: "спрашивать пароль", password: "пароль (keyring)", keepass: "пароль из KeePass" };
const BOOLS = ["fullscreen", "multimon", "dynamic_resolution", "clipboard", "audio", "admin"] as const;
const DEFAULTS: Partial<Profile> = { port: 3389, width: 1280, height: 720, scale: 100, cert: "deny", auth: "prompt", dynamic_resolution: true };

/** Password is never trimmed: leading/trailing spaces may be part of the secret. */
function passwordPrompt(name: string): Promise<string | null> {
  const dlg = document.createElement("dialog");
  dlg.className = "rdp-password";
  dlg.innerHTML = `<form method="dialog"><h3>Пароль RDP: ${esc(name)}</h3><label>Пароль <input type="password" name="password" required autocomplete="off" /></label><div class="actions"><button value="cancel" formnovalidate>Отмена</button><button value="connect" class="primary">Подключиться</button></div></form>`;
  document.body.append(dlg);
  overlay(true);
  return new Promise((resolve) => {
    dlg.onclose = () => {
      const input = dlg.querySelector("input")!;
      const password = dlg.returnValue === "connect" ? input.value : null;
      input.value = "";
      dlg.remove();
      overlay(false);
      resolve(password);
    };
    dlg.showModal();
    dlg.querySelector("input")!.focus();
  });
}
async function connect(p: Profile) {
  let password: string | null = null;
  try {
    if (p.auth === "prompt") { password = await passwordPrompt(p.name); if (password === null) return; }
    await invoke<Session>("rdp_connect", { id: p.id, password });
    window.dispatchEvent(new Event("rdp-sessions-changed"));
    toast("FreeRDP запущен отдельным окном");
  } catch (e) { toast(String(e), "err"); }
  finally { password = null; }
}
registerProvider(async () => (await invoke<Profile[]>("rdp_list")).map((p) => ({ group: "RDP", title: `RDP: ${p.name}`, hint: `${p.group ? p.group + " · " : ""}${p.host}:${p.port}`, run: () => connect(p) })));

export function mountRdp(root: HTMLElement) {
  root.innerHTML = `<div class="page">
    <div class="page-head"><h2>RDP · FreeRDP 3</h2><div class="row"><input class="rdp-filter" placeholder="фильтр…" /><button class="primary" data-a="add">${icon("plus", 16)} Хост</button>${helpBtn("rdp")}</div></div>
    <p class="muted">Подключение открывает отдельное окно xfreerdp3 (X11 / XWayland). Пароль передаётся напрямую FreeRDP и не копируется в буфер.</p>
    <p class="rdp-status muted" role="status"></p><div class="rdp-sessions"></div><div class="rdp-list"></div>
    <dialog class="rdp-dialog"><form method="dialog"><h3>RDP-хост</h3>
      <div class="grid2">
        <label>Название <input name="name" required /></label><label>Группа <input name="group" list="rdp-groups" /></label>
        <label>Адрес <input name="host" required spellcheck="false" placeholder="host.example.com / IPv4 / IPv6" /></label><label>Порт <input name="port" type="number" min="1" max="65535" required /></label>
        <label>Пользователь <input name="user" autocomplete="off" spellcheck="false" /></label><label>Домен <input name="domain" autocomplete="off" spellcheck="false" /></label>
      </div>
      <label>Аутентификация <select name="auth">${Object.entries(AUTH).map(([k, v]) => `<option value="${k}">${v}</option>`).join("")}</select></label>
      <div data-s="keepass" class="kp-bind"><span class="kp-bound muted"></span><button type="button" data-a="pick">Выбрать запись…</button></div>
      <label data-s="password">Пароль <input name="secret" type="password" autocomplete="new-password" /><span class="muted">Пусто — оставить сохранённый пароль.</span></label>
      <label class="check" data-s="password"><input type="checkbox" name="clear_secret" /> Удалить сохранённый пароль</label>
      <div class="grid2"><label>Ширина <input name="width" type="number" min="200" max="8192" required /></label><label>Высота <input name="height" type="number" min="200" max="8192" required /></label></div>
      <label>Масштаб <select name="scale"><option>100</option><option>140</option><option>180</option></select></label>
      <label>Сертификат <select name="cert"><option value="deny">Строгая проверка (рекомендуется)</option><option value="tofu">Доверять первому сертификату (TOFU)</option><option value="ignore">Не проверять (небезопасно)</option></select></label>
      <p class="muted">TOFU доверяет первому подключению. Без проверки возможен перехват соединения.</p>
      <div class="grid2">${[["fullscreen", "Полный экран"], ["multimon", "Все мониторы"], ["dynamic_resolution", "Разрешение по размеру окна"], ["clipboard", "Общий буфер обмена"], ["audio", "Звук"], ["admin", "Сессия администратора"]].map(([k, v]) => `<label class="check"><input type="checkbox" name="${k}" /> ${v}</label>`).join("")}</div>
      <datalist id="rdp-groups"></datalist><p class="err form-err" role="alert"></p><div class="actions"><button value="cancel" formnovalidate>Отмена</button><button value="save" class="primary">Сохранить</button></div>
    </form></dialog></div>`;
  const list = root.querySelector<HTMLElement>(".rdp-list")!;
  const filter = root.querySelector<HTMLInputElement>(".rdp-filter")!;
  const dialog = root.querySelector<HTMLDialogElement>(".rdp-dialog")!;
  const form = dialog.querySelector("form")!;
  const f = (key: string) => form.elements.namedItem(key) as HTMLInputElement & HTMLSelectElement;
  let profiles: Profile[] = [], editing: Profile | null = null, bound = "";
  let titles = new Map<string, string>();
  let status: Status = { available: false, version: "Проверка FreeRDP…" };
  const closed = new Set<string>((() => { try { return JSON.parse(localStorage.getItem("opsdeck.rdp.closed") ?? "[]"); } catch { return []; } })());
  const saveClosed = () => localStorage.setItem("opsdeck.rdp.closed", JSON.stringify([...closed]));
  const sync = () => {
    form.querySelectorAll<HTMLElement>("[data-s]").forEach((el) => el.hidden = el.dataset.s !== f("auth").value);
    f("user").required = f("auth").value !== "keepass";
    f("user").placeholder = f("auth").value === "keepass" ? "из записи KeePass" : "";
  };
  f("auth").onchange = sync;
  const setBound = (id: string, title?: string) => { bound = id; form.querySelector(".kp-bound")!.textContent = id ? title ?? titles.get(id) ?? "запись выбрана" : "запись не выбрана"; };
  form.querySelector<HTMLElement>("[data-a=pick]")!.onclick = async () => { const e = await pickEntry(); if (e) setBound(e.id, e.title); };
  function open(p: Profile | null) {
    editing = p;
    form.reset();
    form.querySelector(".form-err")!.textContent = "";
    for (const k of ["name", "group", "host", "port", "user", "domain", "auth", "width", "height", "scale", "cert"] as const) f(k).value = String(p?.[k] ?? DEFAULTS[k] ?? "");
    BOOLS.forEach((k) => f(k).checked = Boolean(p?.[k] ?? DEFAULTS[k]));
    setBound(p?.keepass_entry ?? "");
    form.querySelector("datalist")!.innerHTML = [...new Set(profiles.map((p) => p.group).filter(Boolean))].map((g) => `<option value="${esc(g)}">`).join("");
    sync(); overlay(true); dialog.showModal();
  }
  dialog.onclose = () => { f("secret").value = ""; f("clear_secret").checked = false; overlay(false); };
  form.addEventListener("submit", async (event) => {
    if ((event.submitter as HTMLButtonElement)?.value !== "save") return;
    event.preventDefault();
    const profile: Profile = {
      id: editing?.id ?? crypto.randomUUID(), name: f("name").value.trim(), group: f("group").value.trim(), host: f("host").value.trim(), port: Number(f("port").value), user: f("user").value.trim(), domain: f("domain").value.trim(), auth: f("auth").value, keepass_entry: f("auth").value === "keepass" ? bound : "", width: Number(f("width").value), height: Number(f("height").value), scale: Number(f("scale").value), cert: f("cert").value,
      fullscreen: f("fullscreen").checked, multimon: f("multimon").checked, dynamic_resolution: f("dynamic_resolution").checked, clipboard: f("clipboard").checked, audio: f("audio").checked, admin: f("admin").checked,
    };
    const btn = event.submitter as HTMLButtonElement;
    btn.disabled = true;
    try {
      await invoke("rdp_save", { profile, secret: profile.auth === "password" ? f("secret").value || null : null, clearSecret: f("clear_secret").checked });
      dialog.close(); await load();
    } catch (e) { form.querySelector(".form-err")!.textContent = String(e); }
    finally { f("secret").value = ""; btn.disabled = false; }
  });
  function draw() {
    const q = filter.value.trim().toLowerCase();
    const groups = new Map<string, Profile[]>();
    profiles.filter((p) => `${p.name} ${p.host} ${p.user} ${p.domain} ${p.group}`.toLowerCase().includes(q)).sort((a, b) => a.name.localeCompare(b.name)).forEach((p) => groups.set(p.group, [...(groups.get(p.group) ?? []), p]));
    list.innerHTML = [...groups].sort(([a], [b]) => a.localeCompare(b)).map(([group, hosts]) => `<details class="ssh-group" data-g="${esc(group)}" ${q || !closed.has(group) ? "open" : ""}><summary><span class="ssh-group-title">${esc(group || "Без группы")}</span><span class="conn-row-count">${hosts.length}</span>${group ? `<button class="icon" data-a="rename" title="Переименовать группу">${icon("edit", 14)}</button>` : ""}</summary><table class="res mt-table"><tbody>${hosts.map((p) => `<tr data-id="${esc(p.id)}"><td class="mt-name">${esc(p.name)}</td><td class="mono">${esc(p.user)}${p.user ? "@" : ""}${esc(p.host)}:${p.port}</td><td class="muted">${esc(p.auth === "keepass" ? titles.get(p.keepass_entry) ?? "KeePass" : AUTH[p.auth])}</td><td class="mt-acts"><button class="primary" data-a="connect" ${status.available ? "" : "disabled"}>Подключиться</button><button class="icon" data-a="edit" title="Изменить профиль и группу">${icon("edit", 14)}</button><button class="icon danger" data-a="del" title="Удалить">${icon("trash", 14)}</button></td></tr>`).join("")}</tbody></table></details>`).join("") || `<p class="muted">${q ? "Ничего не найдено." : "Пока пусто — добавьте RDP-хост."}</p>`;
    list.querySelectorAll("details").forEach((d) => d.ontoggle = () => { if (!q) { d.open ? closed.delete(d.dataset.g!) : closed.add(d.dataset.g!); saveClosed(); } });
  }
  async function sessions() {
    const active = await invoke<Session[]>("rdp_sessions");
    root.querySelector(".rdp-sessions")!.innerHTML = active.map((s) => `<div class="row"><span>RDP: ${esc(s.name)} · PID ${s.pid}</span><button data-stop="${esc(s.id)}">Отключить</button></div>`).join("");
  }
  async function load() {
    try {
      [profiles, status] = await Promise.all([invoke<Profile[]>("rdp_list"), invoke<Status>("rdp_status")]);
      root.querySelector(".rdp-status")!.textContent = status.version;
      titles = new Map();
      if (profiles.some((p) => p.auth === "keepass") && (await kpStatus()).unlocked) (await kpEntries()).forEach((e) => titles.set(e.id, e.title));
      draw(); await sessions();
    } catch (e) { toast(String(e), "err"); }
  }
  list.onclick = (event) => { void (async () => {
    const b = (event.target as HTMLElement).closest<HTMLElement>("[data-a]");
    if (!b) return;
    if (b.dataset.a === "rename") {
      event.preventDefault();
      const from = b.closest<HTMLElement>("details")!.dataset.g!;
      const to = await ask("Переименовать группу", `Новое имя для «${from}» (пусто — без группы):`, { input: from });
      if (to === null) return;
      await invoke("rdp_group_rename", { from, to });
      if (closed.delete(from) && to) closed.add(to); saveClosed(); await load(); return;
    }
    const p = profiles.find((p) => p.id === b.closest<HTMLElement>("tr")?.dataset.id);
    if (!p) return;
    if (b.dataset.a === "connect") await connect(p);
    if (b.dataset.a === "edit") open(p);
    if (b.dataset.a === "del" && await ask("Удалить RDP-хост", `Удалить «${p.name}» и пароль из keyring?`, { danger: true, ok: "Удалить" }) !== null) { await invoke("rdp_delete", { id: p.id }); await load(); }
  })().catch((e) => toast(String(e), "err")); };
  root.querySelector<HTMLElement>(".rdp-sessions")!.onclick = (e) => {
    const id = (e.target as HTMLElement).closest<HTMLElement>("[data-stop]")?.dataset.stop;
    if (id) void invoke("rdp_disconnect", { id }).then(sessions).catch((e) => toast(String(e), "err"));
  };
  filter.oninput = draw;
  root.querySelector<HTMLElement>("[data-a=add]")!.onclick = () => open(null);
  window.addEventListener("view-shown", (e) => { if ((e as CustomEvent).detail === "rdp") void load(); });
  window.addEventListener("rdp-sessions-changed", () => { void sessions().catch(() => {}); });
  void listen<{ success: boolean; code: number | null }>("rdp-exit", (e) => {
    if (!e.payload.success) toast(`FreeRDP завершился с кодом ${e.payload.code ?? "?"}. Проверьте адрес, пароль, сертификат и XWayland.`, "err");
    void sessions().catch(() => {});
  }).catch(() => {});
}
