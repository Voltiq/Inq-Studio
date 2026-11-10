fn main() {
    // The icon is compiled into the executable's resources. Cargo doesn't
    // track it on its own, so without this a new icon never shows up.
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=icons/32x32.png");
    println!("cargo:rerun-if-changed=icons/icon.png");
    tauri_build::build()
}
