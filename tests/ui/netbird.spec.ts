import { test, expect } from "./fixtures";

test("native profiles connect by ID, and the server/name editor saves no credentials", async ({ app, page }) => {
  await app.view("netbird");
  await expect(page.locator('.netbird-list tr[data-nb-id="1234abcd"]')).toContainText("Office");
  await page.locator('.netbird-list tr[data-nb-id="1234abcd"] [data-nb=connect]').click();
  expect((await app.called("netbird_connect")).args).toEqual({ id: "1234abcd" });
  await page.locator('[data-nb=add]').click();
  const dlg = page.locator(".netbird-dialog");
  await dlg.locator('[name=name]').fill("Office 2");
  await dlg.locator('[name=management_url]').fill("https://vpn.example.org:443");
  await dlg.locator('[name=admin_url]').fill("https://app.example.org");
  await dlg.locator('[value=save]').click();
  expect((await app.called("netbird_save")).args).toEqual({ id: null, name: "Office 2", servers: { management_url: "https://vpn.example.org:443", admin_url: "https://app.example.org" } });
  await expect(dlg).toBeHidden();
  await page.locator('.netbird-list tr[data-nb-id="1234abcd"] [data-nb=edit]').click();
  await expect(dlg.locator('[name=management_url]')).toHaveValue("https://vpn.example.com");
  await dlg.locator('[name=name]').fill("Renamed");
  await dlg.locator('[value=save]').click();
  await expect.poll(async () => (await app.calls("netbird_save")).at(-1)?.args).toMatchObject({ id: "1234abcd", name: "Renamed" });
});

test("switching/disconnecting needs confirmation; active/default cannot be removed", async ({ app, page }) => {
  await app.override("netbird_snapshot", { profiles: [{ id: "default", name: "Office", active: true }, { id: "1234abcd", name: "Office", active: false }], status: { connected: true, state: "Connected", management_url: "https://vpn.example.com", ip: "100.64.1.2" } });
  await app.view("netbird");
  await expect(page.locator('tr[data-nb-id=default] [data-nb=remove]')).toBeDisabled();
  await page.locator('tr[data-nb-id="1234abcd"] [data-nb=connect]').click();
  await page.locator('dialog.ask button[value=cancel]').click();
  expect(await app.calls("netbird_connect")).toHaveLength(0);
  await page.locator('[data-nb=disconnect]').click();
  await page.locator('dialog.ask .primary').click();
  await app.called("netbird_disconnect");
});

test("daemon error is visible and does not crash the module", async ({ app, page }) => {
  await page.evaluate(() => { (window as any).__DEMO_OVERRIDES.netbird_snapshot = () => { throw new Error("NetBird daemon unavailable"); }; });
  await app.view("netbird");
  await expect(page.locator(".netbird-error")).toContainText("daemon unavailable");
});

test("in-progress connect disables repeat operations", async ({ app, page }) => {
  await page.evaluate(() => { (window as any).__DEMO_OVERRIDES.netbird_connect = () => new Promise((r) => { (window as any).__finishNetbird = r; }); });
  await app.view("netbird");
  await page.locator('tr[data-nb-id="1234abcd"] [data-nb=connect]').click();
  await expect(page.locator('[data-nb=add]')).toBeDisabled();
  await expect(page.locator('tr[data-nb-id="1234abcd"] [data-nb=connect]')).toBeDisabled();
  await page.evaluate(() => (window as any).__finishNetbird());
  await expect(page.locator('[data-nb=add]')).toBeEnabled();
});
