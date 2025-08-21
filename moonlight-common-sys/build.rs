use std::path::PathBuf;

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

    if let Ok(ssl_root_dir) = std::env::var("DEP_OPENSSL_ROOT") {
        config.define("OPENSSL_INCLUDE_DIR", format!("{ssl_root_dir}/include"));

        // TODO: file extension .a or .lib
        config.define(
            "OPENSSL_CRYPTO_LIBRARY",
            format!("{ssl_root_dir}/lib/libcrypto.a"),
        );

        // -- For Cross:
        // config.define("CMAKE_CROSSCOMPILING", "TRUE");
        // config.define("OPENSSL_USE_STATIC_LIBS", "TRUE");

        // IMPORTANT: Skip the compile-and-run checks that fail in cross-compilation
        // config.define("OPENSSL_NO_VERIFY", "TRUE");

        // Cross-compilation flags
        // config.define("CMAKE_FIND_ROOT_PATH_MODE_PROGRAM", "NEVER");
        // config.define("CMAKE_FIND_ROOT_PATH_MODE_PACKAGE", "BOTH");
        // config.define("CMAKE_FIND_ROOT_PATH_MODE_LIBRARY", "ONLY");
        // config.define("CMAKE_FIND_ROOT_PATH_MODE_INCLUDE", "BOTH");
        // config.define("CMAKE_TRY_COMPILE_TARGET_TYPE", "STATIC_LIBRARY");

        // Prevent CMake try_compile
        // config.define("OPENSSL_FOUND", "TRUE");
    }

    let profile = config.get_profile().to_string();
    (profile, config.build())
}

fn link(extra: Option<(String, PathBuf)>) {
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
