fn main() {
  println!("cargo:rerun-if-changed=../renderer");
  tauri_build::build()
}
