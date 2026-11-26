//! Builds `locust-apple-model`, the Swift helper behind the "apple" runtime (helpers/apple-model/main.swift), and
//! leaves it in OUT_DIR for `runtime::apple` to embed. Only on macOS, and only where the Swift compiler and the
//! Foundation Models framework are present; elsewhere the runtime is simply not offered (cfg `apple_helper` unset).
//!
//! Also passes the commit a build was made from through to `build_info`: `LOCUST_BUILD_COMMIT` when the build is given
//! it, else `dev`. Git is never read and `.git` never watched, so an ordinary build does not rebuild on every commit.
use std::{env, path::PathBuf, process::Command};

#[path = "build_commit.rs"]
mod build_commit;

fn main() {
    println!("cargo::rerun-if-env-changed=LOCUST_BUILD_COMMIT");
    let given = env::var("LOCUST_BUILD_COMMIT").ok();
    println!("cargo::rustc-env=LOCUST_BUILD_COMMIT={}", build_commit::given_commit(given.as_deref()));
    println!("cargo::rustc-check-cfg=cfg(apple_helper)");
    println!("cargo::rerun-if-changed=helpers/apple-model/main.swift");
    println!("cargo::rerun-if-env-changed=LOCUST_NO_APPLE_HELPER");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") || env::var_os("LOCUST_NO_APPLE_HELPER").is_some() {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR")).join("locust-apple-model");
    let built = Command::new("xcrun")
        .args(["swiftc", "-O", "-suppress-warnings", "helpers/apple-model/main.swift", "-o"])
        .arg(&out)
        .status();
    match built {
        Ok(status) if status.success() => println!("cargo::rustc-cfg=apple_helper"),
        Ok(status) => println!("cargo::warning=locust-apple-model was not built ({status}); the apple runtime is left out"),
        Err(e) => println!("cargo::warning=locust-apple-model was not built ({e}); the apple runtime is left out"),
    }
}
