import { test, expect } from './fixtures';
const profile = { id: 'rdp-1', name: 'win-prod', group: 'prod', host: '192.0.2.10', port: 3389, user: 'alice', domain: 'AD', auth: 'prompt', keepass_entry: '', width: 1280, height: 720, scale: 100, cert: 'deny', fullscreen: false, multimon: false, dynamic_resolution: true, clipboard: false, audio: false, admin: false };
test.use({ demo: { overrides: { rdp_list: [profile] } } });
test.beforeEach(async ({ app }) => { await app.view('rdp'); });
test('groups, filter, external connection and password cleanup', async ({ page, app }) => {
  await expect(page.locator('.rdp-list details[data-g=prod] tr')).toHaveCount(1);
  await page.locator('.rdp-filter').fill('missing');
  await expect(page.locator('.rdp-list tr')).toHaveCount(0);
  await page.locator('.rdp-filter').fill('192.0.2');
  await page.locator('.rdp-list [data-a=connect]').click();
  const prompt = page.locator('dialog.rdp-password');
  await expect(prompt.locator('input')).toHaveAttribute('type', 'password');
  await prompt.locator('input').fill('  secret : $ " \\  ');
  await prompt.locator('[value=connect]').click();
  expect((await app.called('rdp_connect')).args).toEqual({ id: 'rdp-1', password: '  secret : $ " \\  ' });
  await expect(prompt).toHaveCount(0);
  expect(await app.calls('pty_spawn')).toHaveLength(1); // only startup terminal; RDP has no PTY
  await expect(page.locator('#sidebar [data-view=rdp]')).toHaveClass(/active/);
});
test('cancel never starts a process', async ({ page, app }) => {
  await page.locator('.rdp-list [data-a=connect]').click();
  await page.locator('dialog.rdp-password [value=cancel]').click();
  await expect(page.locator('dialog.rdp-password')).toHaveCount(0);
  expect(await app.calls('rdp_connect')).toHaveLength(0);
});
test('profile settings and keyring save; secret input cleared', async ({ page, app }) => {
  await page.locator('.rdp-list [data-a=edit]').click();
  const dlg = page.locator('.rdp-dialog');
  await dlg.locator('[name=auth]').selectOption('password');
  await dlg.locator('[name=secret]').fill('keyring-secret');
  await dlg.locator('[name=width]').fill('1920');
  await dlg.locator('[name=clipboard]').check();
  await dlg.locator('[name=group]').fill('office');
  await dlg.locator('[value=save]').click();
  expect((await app.called('rdp_save')).args).toMatchObject({ profile: { id: 'rdp-1', group: 'office', width: 1920, clipboard: true, cert: 'deny', auth: 'password' }, secret: 'keyring-secret', clearSecret: false });
  await expect(dlg).toBeHidden();
  await expect(dlg.locator('[name=secret]')).toHaveValue('');
});
test('KeePass connection sends only profile id and no password', async ({ page, app }) => {
  await app.override('rdp_list', [{ ...profile, auth: 'keepass', keepass_entry: 'k1' }]);
  await app.view('ssh'); await app.view('rdp');
  await page.locator('.rdp-list [data-a=connect]').click();
  expect((await app.called('rdp_connect')).args).toEqual({ id: 'rdp-1', password: null });
  await expect(page.locator('dialog.rdp-password')).toHaveCount(0);
});
test('missing FreeRDP disables connection', async ({ page, app }) => {
  await app.override('rdp_status', { available: false, version: 'Установите FreeRDP 3' });
  await app.view('ssh'); await app.view('rdp');
  await expect(page.locator('.rdp-list [data-a=connect]')).toBeDisabled();
  await expect(page.locator('.rdp-status')).toContainText('Установите FreeRDP 3');
});
test('session disconnect passes session identity, not PID', async ({ page, app }) => {
  await app.override('rdp_sessions', [{ id: 'session-1', profile_id: profile.id, name: profile.name, pid: 1234 }]);
  await app.view('ssh'); await app.view('rdp');
  await page.locator('[data-stop=session-1]').click();
  expect((await app.called('rdp_disconnect')).args).toEqual({ id: 'session-1' });
});
