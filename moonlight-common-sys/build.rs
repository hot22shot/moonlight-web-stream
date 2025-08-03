use std::{env, path::PathBuf};

fn main() {
    #[cfg(feature = "generate-bindings")]
    generate_bindings();

    #[allow(unused)]
    let moonlight_output: Option<(String, PathBuf)> = None;

    #[cfg(feature = "build-moonlight_common_c")]
    let moonlight_output = Some(compile_moonlight());

    link(moonlight_output);
}

#[cfg(feature = "generate-bindings")]
fn generate_bindings() {
    generate_bindings_with_name("limelight.h", "limelight.rs");
    #[cfg(feature = "crypto")]
    generate_bindings_with_name("crypto.h", "crypto.rs");
}
#[cfg(feature = "generate-bindings")]
fn generate_bindings_with_name(header_name: &str, rust_name: &str) {
    let bindings = bindgen::Builder::default()
        .header(header_name)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join(rust_name))
        .expect("Couldn't write bindings!");
}

#[cfg(feature = "build-moonlight_common_c")]
fn compile_moonlight() -> (String, PathBuf) {
    // builds into $OUT_DIR
    let mut config = cmake::Config::new("moonlight-common-c");
    config.define("BUILD_SHARED_LIBS", "OFF");

    let profile = config.get_profile().to_string();
    (profile, config.build())
}

fn link(extra: Option<(String, PathBuf)>) {
    // OpenSSL, crypto
    println!("cargo:rustc-link-lib=static=libcrypto");

    // ENet
    #[cfg(feature = "link-enet")]
    {
        if let Some((profile, path)) = &extra {
            println!(
                "cargo:rustc-link-search=native={}/build/enet/{profile}",
                path.display()
            );
        }
        println!("cargo:rustc-link-lib=static=enet");
    }

    // Moonlight
    #[cfg(feature = "link-moonlight_common_c")]
    {
        let (profile, path) = extra.expect("moonlight build output path");
        println!(
            "cargo:rustc-link-search=native={}/build/{profile}",
            path.display(),
        );
        println!("cargo:rustc-link-lib=static=moonlight-common-c");
    }

    // Windows Debug: msvcrtd.lib
    #[cfg(all(target_os = "windows", debug_assertions))]
    if cfg!(debug_assertions) {
        println!("cargo:rustc-link-lib=dylib=msvcrtd");
    }
}
