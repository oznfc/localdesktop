fn main() {
    let lib_path = "./assets/libs/arm64-v8a";
    println!("cargo::rustc-link-search={}", lib_path);
}
