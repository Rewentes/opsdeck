import { invoke } from "@tauri-apps/api/core";
import { ask, esc, overlay, toast } from "./ui";
import { icon } from "./icons";
import { registerProvider } from "./palette";

export type NetbirdProfile = { id: string; name: string; active: boolean; management_url: string; admin_url: string };
export type NetbirdSnapshot = { profiles: NetbirdProfile[]; status: { connected: boolean; state: string; management_url: string; ip: string } };
export const netbirdSnapshot = () => invoke<NetbirdSnapshot>("netbird_snapshot");
export async function confirmNetbird(id: string): Promise<boolean> {
  const snap = await netbirdSnapshot();
  if (!snap.profiles.some((p) => p.id === id)) throw new Error("Привязанный профиль NetBird не найден");
  const current = snap.profiles.find((p) => p.active);
  const target = snap.profiles.find((p) => p.id === id)!;
  const changesServer = target.management_url && new URL(target.management_url).href !== new URL(snap.status.management_url || target.management_url).href;
  if (snap.status.connected && current && (current.id !== id || changesServer)) {
    return await ask("Переключить NetBird", `Сейчас подключён «${current.name}». Переключение прервёт соединения через эту сеть. Продолжить?`, { ok: "Переключить" }) !== null;
  }
  return true;
}
registerProvider(async () => (await netbirdSnapshot()).profiles.map((p) => ({ group: "NetBird", title: `NetBird: ${p.name}`, hint: p.id, run: async () => {
  try { if (await confirmNetbird(p.id)) { toast("NetBird подключается. При необходимости подтвердите вход в браузере"); await invoke("netbird_connect", { id: p.id }); toast("NetBird подключён"); window.dispatchEvent(new Event("netbird-changed")); } }
  catch (e) { toast(String(e), "err"); }
} })));

export function mountNetbird(root: HTMLElement) {
  root.innerHTML = `<div class="page netbird-page"><div class="page-head"><h2>NetBird</h2><div class="row"><button data-nb="refresh">${icon("refresh", 16)} Обновить</button><button class="primary" data-nb="add">${icon("plus", 16)} Профиль</button></div></div>
    <p class="muted">Одновременно работает одна сеть. При смене профиля прежнее соединение отключается. Вход подтверждается в браузере NetBird; ключи и токены хранит сама служба.</p>
    <p class="netbird-status" role="status"></p><p class="netbird-error err" role="alert"></p><div class="netbird-list"></div>
    <dialog class="netbird-dialog"><form method="dialog"><h3>Профиль NetBird</h3><label>Название <input name="name" required maxlength="128" /></label>
      <label>URL сервера авторизации (Management URL) <input name="management_url" type="url" placeholder="https://api.netbird.io:443" /></label>
      <label>URL панели NetBird <input name="admin_url" type="url" placeholder="https://app.netbird.io:443" /></label>
      <p class="muted">Пустой URL сохраняет сервер самого NetBird. Для нового профиля по умолчанию используется NetBird Cloud. Изменение URL применяется при следующем подключении.</p>
      <p class="netbird-form-error err" role="alert"></p><div class="actions"><button value="cancel" formnovalidate>Отмена</button><button value="save" class="primary">Сохранить</button></div></form></dialog></div>`;
  const dlg = root.querySelector<HTMLDialogElement>(".netbird-dialog")!;
  const form = dlg.querySelector("form")!;
  const field = (name: string) => form.elements.namedItem(name) as HTMLInputElement;
  let snapshot: NetbirdSnapshot | null = null, editing: NetbirdProfile | null = null, busy = false;
  const list = root.querySelector<HTMLElement>(".netbird-list")!;
  const error = root.querySelector<HTMLElement>(".netbird-error")!;
  function draw() {
    if (!snapshot) return;
    const active = snapshot.profiles.find((p) => p.active);
    root.querySelector(".netbird-status")!.innerHTML = busy ? "NetBird подключается. При необходимости подтвердите вход в браузере" : `<span>${snapshot.status.connected ? "Подключён" : snapshot.status.state === "NeedsLogin" ? "Требуется вход" : "Отключён"}</span> · <span data-no-i18n>${esc(active?.name ?? "—")} · ${esc(snapshot.status.ip || snapshot.status.management_url)}</span>`;
    list.innerHTML = `<table class="res"><thead><tr><th>Профиль</th><th>ID</th><th>Сервер</th><th></th></tr></thead><tbody>${snapshot.profiles.map((p) => `<tr data-nb-id="${esc(p.id)}"><td data-no-i18n>${esc(p.name)} ${p.active ? "●" : ""}</td><td class="mono" data-no-i18n>${esc(p.id)}</td><td class="mono" data-no-i18n>${esc(p.management_url || (p.active ? snapshot!.status.management_url : ""))}</td><td class="mt-acts"><button data-nb="connect" ${busy ? "disabled" : ""}>Подключить</button><button data-nb="edit" ${busy ? "disabled" : ""}>Изменить</button><button data-nb="remove" ${busy || p.active || p.id === "default" ? "disabled" : ""}>Удалить</button></td></tr>`).join("")}</tbody></table><button data-nb="disconnect" ${busy ? "disabled" : ""}>Отключить NetBird</button>`;
    root.querySelector<HTMLButtonElement>("[data-nb=add]")!.disabled = busy;
  }
  async function load() {
    try { snapshot = await netbirdSnapshot(); error.textContent = ""; draw(); }
    catch (e) { error.textContent = String(e); }
  }
  function open(p: NetbirdProfile | null) {
    editing = p; form.reset();
    field("name").value = p?.name ?? "";
    field("management_url").value = p?.management_url || (p?.active ? snapshot?.status.management_url ?? "" : "");
    field("admin_url").value = p?.admin_url ?? "";
    form.querySelector(".netbird-form-error")!.textContent = "";
    overlay(true); dlg.showModal();
  }
  dlg.onclose = () => overlay(false);
  form.onsubmit = async (e) => {
    if ((e.submitter as HTMLButtonElement)?.value !== "save") return;
    e.preventDefault(); const b = e.submitter as HTMLButtonElement; b.disabled = true;
    try { await invoke("netbird_save", { id: editing?.id ?? null, name: field("name").value.trim(), servers: { management_url: field("management_url").value.trim(), admin_url: field("admin_url").value.trim() } }); dlg.close(); await load(); window.dispatchEvent(new Event("netbird-changed")); }
    catch (err) { form.querySelector(".netbird-form-error")!.textContent = String(err); }
    finally { b.disabled = false; }
  };
  root.onclick = (event) => { void (async () => {
    const b = (event.target as HTMLElement).closest<HTMLButtonElement>("[data-nb]"); if (!b || b.disabled) return;
    const action = b.dataset.nb;
    if (action === "refresh") { await load(); return; }
    if (action === "add") { open(null); return; }
    const p = snapshot?.profiles.find((p) => p.id === b.closest<HTMLElement>("[data-nb-id]")?.dataset.nbId);
    if (action === "edit" && p) { open(p); return; }
    if (busy) return;
    if (action === "connect" && p) {
      if (!await confirmNetbird(p.id)) return;
      busy = true; draw();
      try { await invoke("netbird_connect", { id: p.id }); }
      finally { busy = false; await load(); }
    } else if (action === "disconnect") {
      if (await ask("Отключить NetBird", "Это прервёт соединения через NetBird, включая RDP. Продолжить?", { ok: "Отключить" }) === null) return;
      busy = true; draw(); try { await invoke("netbird_disconnect"); } finally { busy = false; await load(); }
    } else if (action === "remove" && p) {
      if (await ask("Удалить профиль NetBird", `Удалить «${p.name}» и его локальные ключи NetBird?`, { danger: true, ok: "Удалить" }) === null) return;
      await invoke("netbird_remove", { id: p.id }); await load();
    }
    window.dispatchEvent(new Event("netbird-changed"));
  })().catch((e) => { error.textContent = String(e); }); };
  window.addEventListener("view-shown", (e) => { if ((e as CustomEvent).detail === "netbird") void load(); });
  window.addEventListener("netbird-changed", () => { if (!root.hidden) void load(); });
  // Stop polling hidden modules; SSO/network state may also change in the native NetBird UI.
  setInterval(() => { if (!root.hidden && !dlg.open) void load(); }, 5000);
}
