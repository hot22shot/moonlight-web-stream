use std::{
    env,
    path::{Path, PathBuf},
};

fn main() {
    #[cfg(feature = "generate-bindings")]
    generate_bindings();

    #[allow(unused)]
    let moonlight_output: Option<&PathBuf> = None;

    #[cfg(feature = "build-moonlight_common_c")]
    let moonlight_output = Some(compile_moonlight());

    link(moonlight_output.as_deref());
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
fn compile_moonlight() -> PathBuf {
    // builds into $OUT_DIR
    cmake::Config::new("moonlight-common-c")
        .define("BUILD_SHARED_LIBS", "OFF")
        .build()
}

fn link(moonlight_path: Option<&Path>) {
    // OpenSSL, crypto
    #[cfg(feature = "link-openssl")]
    {
        println!(
            "cargo:rustc-link-search=native={}",
            env::var("OPENSSL_LIB_DIR").unwrap()
        );
        println!("cargo:rustc-link-lib=static=libcrypto");
    }

    // ENet
    #[cfg(feature = "link-enet")]
    {
        if let Some(path) = moonlight_path {
            println!(
                "cargo:rustc-link-search=native={}/build/enet/Release",
                path.display()
            );
        }
        println!("cargo:rustc-link-lib=static=enet");
    }

    // Moonlight
    #[cfg(feature = "link-moonlight_common_c")]
    {
        println!(
            "cargo:rustc-link-search=native={}/build/Release",
            moonlight_path
                .expect("moonlight build output path")
                .display()
        );
        println!("cargo:rustc-link-lib=static=moonlight-common-c");
    }
}
