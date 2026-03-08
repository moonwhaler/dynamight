use std::process::Command;

fn main() {
    // Git commit hash: prefer BUILD_GIT_HASH env (set by Docker), fall back to git
    let git_hash = std::env::var("BUILD_GIT_HASH")
        .ok()
        .filter(|s| !s.is_empty() && s != "unknown")
        .unwrap_or_else(|| {
            Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        });

    // Build date: prefer BUILD_DATE env, fall back to date command
    let build_date = std::env::var("BUILD_DATE")
        .ok()
        .filter(|s| !s.is_empty() && s != "unknown")
        .unwrap_or_else(|| {
            Command::new("date")
                .args(["+%Y-%m-%d"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        });

    println!("cargo:rustc-env=BUILD_GIT_HASH={git_hash}");
    println!("cargo:rustc-env=BUILD_DATE={build_date}");

    // Re-run if git HEAD changes (new commits) or build args change
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/refs/");
    println!("cargo:rerun-if-env-changed=BUILD_GIT_HASH");
    println!("cargo:rerun-if-env-changed=BUILD_DATE");
}
