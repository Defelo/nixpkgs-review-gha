//! Real Nix checks run only in CI after the conventional fixture has exported its closure.
use repo_review::{artifact, contract::*, process, read_json};
use std::{fs, path::PathBuf};

fn fixture() -> (Plan, PlatformResult, PathBuf) {
    let root =
        PathBuf::from(std::env::var("REPO_REVIEW_FIXTURE_ROOT").expect("CI fixture directory"));
    let plan = read_json(&root.join("flake-plan.json")).unwrap();
    let bundle = root.join("flake");
    let report = read_json(&bundle.join("review-result.json")).unwrap();
    artifact::validate_bundle(&plan, &report, &bundle).unwrap();
    (plan, report, bundle)
}

#[test]
#[ignore = "real Nix closure transfer is CI-only"]
fn cache_miss_reaches_copy_after_valid_bundle_validation() {
    let (_, report, _) = fixture();
    let temp = tempfile::tempdir().unwrap();
    let cache = temp.path().join("empty-cache");
    fs::create_dir(&cache).unwrap();
    fs::write(cache.join("nix-cache-info"), "StoreDir: /nix/store\n").unwrap();
    let error = artifact::retrieve(
        &report,
        &format!("file://{}", cache.display()),
        &[],
        &temp.path().join("fresh-store"),
        true,
    )
    .unwrap_err();
    assert!(error.to_string().starts_with("nix failed"), "{error:#}");
}

#[test]
#[ignore = "real Nix signature enforcement is CI-only"]
fn unsigned_and_wrong_key_copies_fail_even_with_consumer_require_sigs_false() {
    let (_, report, bundle) = fixture();
    let temp = tempfile::tempdir().unwrap();
    let secret = temp.path().join("fixture-secret");
    let public = temp.path().join("fixture-public");
    let wrong_secret = temp.path().join("wrong-secret");
    let wrong_public = temp.path().join("wrong-public");
    for (name, private, public) in [
        ("repo-review-fixture", &secret, &public),
        ("repo-review-wrong", &wrong_secret, &wrong_public),
    ] {
        process::run(
            "nix-store",
            &[
                "--generate-binary-cache-key".into(),
                name.into(),
                private.display().to_string(),
                public.display().to_string(),
            ],
            None,
            None,
        )
        .unwrap();
    }
    let config = temp.path().join("consumer.conf");
    fs::write(&config, "require-sigs = false\n").unwrap();
    let key = fs::read_to_string(&public).unwrap().trim().to_owned();
    let wrong_key = fs::read_to_string(&wrong_public).unwrap().trim().to_owned();
    let roots = artifact::roots(&report);
    // Use a disposable file-cache copy, preserving the immutable validated bundle.
    let cache = temp.path().join("cache");
    fs::create_dir(&cache).unwrap();
    let cache_url = format!("file://{}", cache.display());
    let mut export = process::nix_args(&[
        "copy",
        "--from",
        &format!("file://{}", bundle.join("cache").display()),
        "--to",
        &cache_url,
        "--no-check-sigs",
    ]);
    export.extend(roots.clone());
    process::run("nix", &export, None, None).unwrap();
    // Public-copy arguments are identical for HTTPS; only the offline fixture URL differs.
    let copy = |keys: &[String], destination: &str| {
        let mut args =
            artifact::retrieval_args(&cache_url, keys, &temp.path().join(destination), false);
        args.extend(roots.clone());
        assert_eq!(args[0], "copy");
        assert!(!args.iter().any(|a| a == "--no-check-sigs" || a == "build"));
        let mut command = process::command("nix", &args, None);
        command.env("NIX_USER_CONF_FILES", &config);
        process::run_command(command, None)
    };
    assert!(copy(std::slice::from_ref(&key), "unsigned-store").is_err());
    let mut sign = process::nix_args(&[
        "store",
        "sign",
        "--store",
        &cache_url,
        "--key-file",
        secret.to_str().unwrap(),
        "--recursive",
    ]);
    sign.extend(roots.clone());
    process::run("nix", &sign, None, None).unwrap();
    assert!(copy(&[wrong_key], "wrong-key-store").is_err());
    copy(&[key], "signed-store").unwrap();
    let store = format!("local?root={}", temp.path().join("signed-store").display());
    assert_eq!(
        artifact::closure_info(&roots, Some(&store)).unwrap(),
        report.closure
    );
}
