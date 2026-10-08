import { readFileSync, writeFileSync } from 'node:fs';
const version = process.argv[2];
if (!/^\d+\.\d+\.\d+-rdp\.\d+$/.test(version ?? '')) throw new Error('Expected X.Y.Z-rdp.N');
for (const p of ['package.json', 'src-tauri/tauri.conf.json']) {
  const data = JSON.parse(readFileSync(p, 'utf8')); data.version = version;
  writeFileSync(p, JSON.stringify(data, null, 2) + '\n');
}
const cargo = 'src-tauri/Cargo.toml';
writeFileSync(cargo, readFileSync(cargo, 'utf8').replace(/^version = "[^"]+"/m, `version = "${version}"`));
const pkg = 'packaging/arch/PKGBUILD';
writeFileSync(pkg, readFileSync(pkg, 'utf8').replace(/^pkgver=.*$/m, `pkgver=${version.replaceAll('-', '.')}`));
