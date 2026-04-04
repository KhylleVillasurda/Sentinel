fn main() {
    let now = chrono::Utc::now().to_rfc3339();
    println!("cargo:rustc-env=BUILD_TIME={}", now);

    tauri_build::build()
}
