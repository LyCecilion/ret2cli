use std::env;

mod release;

fn main() {
    for variable in ["GITHUB_RUN_NUMBER", "GITHUB_RUN_ATTEMPT", "GITHUB_SHA"] {
        println!("cargo:rerun-if-env-changed={variable}");
    }

    let package_version = env!("CARGO_PKG_VERSION");
    let release_codename = release::codename_for_version(package_version).unwrap_or_else(|| {
        panic!(
            "no release codename is configured for {package_version}; add the new major.minor release line to release.rs and update dist-workspace.toml"
        )
    });
    let run_number = env::var("GITHUB_RUN_NUMBER").ok();
    let run_attempt = env::var("GITHUB_RUN_ATTEMPT").ok();
    let sha = env::var("GITHUB_SHA").ok();
    let metadata = release::github_build_metadata(
        run_number.as_deref(),
        run_attempt.as_deref(),
        sha.as_deref(),
    );
    let version = metadata.map_or_else(
        || package_version.to_owned(),
        |metadata| format!("{package_version}+{metadata}"),
    );

    println!("cargo:rustc-env=RET2CLI_VERSION={version}");
    println!("cargo:rustc-env=RET2CLI_CODENAME={release_codename}");
}
