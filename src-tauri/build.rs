fn main() {
    println!("cargo:rerun-if-env-changed=OPSDECK_PACKAGE_FORMAT");
    tauri_build::build()
}
