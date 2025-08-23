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
    // TODO: also rerun when other vars change
    println!("cargo::rerun-if-changed=moonlight-common-c");
    let mut config = cmake::Config::new("moonlight-common-c");
    config.define("BUILD_SHARED_LIBS", "OFF");
    config.define("CMAKE_TRY_COMPILE_TARGET_TYPE", "STATIC_LIBRARY");

    // -- Link OpenSSL: Some environment variables are exported from openssl-sys for all dependents
    // Include
    let ssl_include = var("DEP_OPENSSL_INCLUDE")
        .or(var("OPENSSL_INCLUDE"))
        .unwrap_or_else(|_| {
            let mut ssl_root = var("DEP_OPENSSL_ROOT").expect("failed to find openssl");
            ssl_root.push_str("/include");

            ssl_root
        });
    config.define("OPENSSL_INCLUDE_DIR", ssl_include);

    // Lib
    let ssl_libs = var("DEP_OPENSSL_LIB")
        .or(var("OPENSSL_LIB_DIR"))
        .unwrap_or_else(|_| {
            let mut ssl_root = var("DEP_OPENSSL_ROOT").expect("failed to find openssl");
            ssl_root.push_str("/lib");

            ssl_root
        });
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

    let crypto_lib = format!("{ssl_libs}/libcrypto.{lib_ext}");
    config.define("OPENSSL_CRYPTO_LIBRARY", &crypto_lib);

    // Force the library used by openssl
    config.define("OPENSSL_USE_STATIC_LIBS", "TRUE");

    // TODO: remove Debug
    // for (key, value) in std::env::vars() {
    //     println!("cargo::warning=ENV {key}: {value}");
    // }

    // Cross compiling with cross
    // TODO: pipe this into the toolchain cmake?
    // TODO: only define this if in cross
    // config.define("CMAKE_DISABLE_FIND_PACKAGE_OpenSSL", "TRUE");
    // Disables actually trying to compile the tests when already set
    // config.define("CMAKE_CROSSCOMPILING", "TRUE");

    // Definitions required for some windows headers to enable them
    // -> qos2.h
    let flags = "-D_WIN32_WINNT=0x0600 -DHAS_PQOS_FLOWID -DHAS_QOS_FLOWID";
    config.cflag(flags);
    config.cxxflag(flags);

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
        println!(
            "cargo:rustc-link-search=native={}/build/enet",
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
        println!("cargo:rustc-link-search=native={}/build", path.display(),);
    } else if let Some(lib_path) = lib_path.as_ref() {
        println!("cargo:rustc-link-search=native={}", lib_path);
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
