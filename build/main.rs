fn main() {
    #[cfg(target_os = "openbsd")] generate_openbsd_bindings();
}

#[cfg(target_os = "openbsd")]
fn generate_openbsd_bindings() {
    use std::path::PathBuf;
    use std::env;

    let bindings = bindgen::Builder::default()
        .clang_macro_fallback()
        .header("build/wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .unwrap_or_else(|e|
            panic!("Unable to generate bindings: {e}")
        );

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("openbsd_bindings.rs"))
        .expect("couldn't write bindings");
}
