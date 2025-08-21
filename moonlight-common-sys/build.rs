use std::env::var;
use std::path::PathBuf;

fn main() {
    #[cfg(feature = "generate-bindings")]
    generate_bindings();

    let allow_vendored = var("MOONLIGHT_COMMON_NO_VENDOR").is_err();

    #[allow(unused)]
    let moonlight_output: Option<(String, PathBuf)> = None;

    #[cfg(feature = "vendored")]
    let moonlight_output = compile_moonlight(allow_vendored);

    link(moonlight_output, allow_vendored);
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

#[cfg(feature = "vendored")]
fn compile_moonlight(allow_vendored: bool) -> Option<(String, PathBuf)> {
    if !allow_vendored {
        return None;
    }

    // builds into $OUT_DIR
    let mut config = cmake::Config::new("moonlight-common-c");
    config.define("BUILD_SHARED_LIBS", "OFF");

    // Exported from openssl-sys for all dependents
    if let Ok(ssl_include) = var("DEP_OPENSSL_INCLUDE") {
        config.define("OPENSSL_INCLUDE_DIR", format!("{ssl_include}/include"));
    }
    if let Ok(sll_lib) = var("DEP_OPENSSL_LIB") {
        let lib_ext = {
            let target_os = var("CARGO_CFG_TARGET_OS").unwrap();
            let target_env = var("CARGO_CFG_TARGET_ENV").unwrap();

            match (target_os.as_str(), target_env.as_str()) {
                ("windows", "msvc") => "lib",
                ("windows", "gnu") => "a",
                // other OSes
                (_, _) => "a",
            }
        };

        config.define(
            "OPENSSL_CRYPTO_LIBRARY",
            format!("{sll_lib}/libcrypto.{lib_ext}"),
        );
    }

    let profile = config.get_profile().to_string();
    Some((profile, config.build()))
}

fn link(compile_info: Option<(String, PathBuf)>, allow_vendored: bool) {
    let lib_path = var("MOONLIGHT_COMMON_LIB").ok();

    // Enet
    if let Some((profile, path)) = &compile_info
        && allow_vendored
    {
        println!(
            "cargo:rustc-link-search=native={}/build/enet/{profile}",
            path.display()
        );
    } else if let Some(lib_path) = lib_path.as_ref() {
        println!("cargo:rustc-link-search=native={}/enet", lib_path);
    }
    println!("cargo:rustc-link-lib=static=enet");

    // Moonlight
    if let Some((profile, path)) = &compile_info
        && allow_vendored
    {
        println!(
            "cargo:rustc-link-search=native={}/build/{profile}",
            path.display(),
        );
    } else if let Some(lib_path) = lib_path.as_ref() {
        println!("cargo:rustc-link-search=native={}", lib_path,);
    }
    println!("cargo:rustc-link-lib=static=moonlight-common-c");

    // Windows Debug: msvcrtd.lib
    let target_os = var("CARGO_CFG_TARGET_OS").unwrap();
    let is_debug = compile_info
        .as_ref()
        .map(|(profile, _)| profile)
        .is_some_and(|profile| profile == "Debug");

    if target_os == "windows" && is_debug {
        println!("cargo:rustc-link-lib=dylib=msvcrtd");
    }
}
