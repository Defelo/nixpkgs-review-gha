use repo_review::{
    artifact::*, backend::*, canonical_digest, contract::*, github::resolve_pr, process, service::*,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const MERGE: &str = "cccccccccccccccccccccccccccccccccccccccc";
const ROOT: &str = "/nix/store/00000000000000000000000000000000-package";
const DEP: &str = "/nix/store/11111111111111111111111111111111-runtime";
fn pr() -> Value {
    json!({"number":1,"head":{"sha":HEAD,"repo":{"full_name":"contributor/project"}},"base":{"sha":BASE},"mergeable":false,"merged":false,"merge_commit_sha":MERGE})
}
fn plan() -> Plan {
    let r = example();
    let identity = Identity {
        id: 1,
        repository: r.repository.clone(),
        commit: HEAD.into(),
        tree: HEAD.into(),
    };
    let mut p = Plan {
        schema_version: VERSION,
        request_id: canonical_digest(&r).unwrap(),
        request: r,
        target: identity.clone(),
        pr: Some(PullRequest {
            number: 1,
            url: "https://github.com/OWNER/REPOSITORY/pull/1".into(),
            head_repository: "contributor/project".into(),
            head: HEAD.into(),
            base: BASE.into(),
            merge: None,
            parents: vec![],
        }),
        recipe: None,
        controller: identity,
        workflow: WORKFLOW.into(),
        tool_lock: json!({}),
        tool_lock_digest: canonical_digest(&json!({})).unwrap(),
        run_repository: "OWNER/REPOSITORY".into(),
        run_id: 1,
        run_attempt: 1,
        digest: String::new(),
    };
    p.seal().unwrap();
    p
}
fn report(p: &Plan) -> PlatformResult {
    let reference = flake_ref(p);
    let mut e = EffectivePlan {
        metadata_digest: p.digest.clone(),
        system: "x86_64-linux".into(),
        flake_reference: Some(reference.clone()),
        lock: Some(json!({"version":7,"root":"root","nodes":{"root":{}}})),
        lock_digest: None,
        source_nar_hash: Some("sha256-test".into()),
        targets: vec![],
        digest: String::new(),
    };
    e.lock_digest = Some(canonical_digest(e.lock.as_ref().unwrap()).unwrap());
    for (kind, name, check) in [("packages", "default", false), ("checks", "smoke", true)] {
        e.targets.push(Target {
            installable: format!("{reference}#{kind}.x86_64-linux.{name}"),
            derivation: format!("{ROOT}.drv"),
            outputs: BTreeMap::from([("out".into(), ROOT.into())]),
            check,
        });
    }
    e.seal().unwrap();
    PlatformResult {
        schema_version: VERSION,
        plan: p.clone(),
        target_outcomes: e
            .targets
            .iter()
            .map(|t| (t.installable.clone(), Outcome::Passed))
            .collect(),
        test_evidence: e
            .targets
            .iter()
            .filter(|t| t.check)
            .map(|t| {
                (
                    t.installable.clone(),
                    "realized; execution-versus-substitution-not-proven".into(),
                )
            })
            .collect(),
        effective: e,
        runner_architecture: "x86_64".into(),
        nix_version: "Nix test fixture".into(),
        sandbox: "true".into(),
        build: Outcome::Passed,
        tests: Outcome::Passed,
        closure_export: Outcome::Passed,
        publication: Outcome::NotRun,
        retrieval: Outcome::NotRun,
        error: None,
        files: BTreeMap::new(),
        closure: BTreeMap::from([(
            ROOT.into(),
            Nar {
                nar_hash: "sha256-test".into(),
                nar_size: 1,
                references: vec![],
            },
        )]),
    }
}
#[test]
fn strict_request_fields_selectors_and_platforms() {
    let r = example();
    r.validate().unwrap();
    let mut v = serde_json::to_value(&r).unwrap();
    v["extra_nix_config"] = json!("sandbox=false");
    assert!(serde_json::from_value::<Request>(v).is_err());
    let mut r = r.clone();
    r.revision = Some("main".into());
    assert!(r.validate().is_err());
    r.revision = None;
    r.pr = Some(0);
    assert!(r.validate().is_err());
    r.pr = Some(1);
    r.systems.clear();
    assert!(r.validate().is_err());
    r.systems = vec!["riscv64-linux".into()];
    assert!(r.validate().is_err());
}
#[test]
fn rejects_path_option_and_token_injection() {
    for s in [
        "--help",
        "../flake",
        "a/b",
        "foo;touch-x",
        "foo\nbar",
        "$(id)",
        "foo.out",
        "",
        "a\"b",
    ] {
        let mut r = example();
        r.packages = vec![s.into()];
        assert!(r.validate().is_err(), "{s}");
    }
    for s in [
        "https://token@github.com/a/b",
        "a/b/c",
        "-a/b",
        "a/../b",
        "https://gitlab.com/a/b",
    ] {
        assert!(repository(s).is_err());
    }
    for s in [
        "https://user:secret@example.com",
        "https://example.com?token=foo",
        "file:///tmp/a",
        "http://example.com",
    ] {
        assert!(public_url(s).is_err());
    }
    for s in [
        "--help",
        "/nix/store/../../etc/passwd",
        "/nix/store/00000000000000000000000000000000-p/bin/x",
    ] {
        assert!(store_path(s).is_err());
    }
}
#[test]
fn zero_targets_and_backend_conflicts_fail() {
    let mut r = example();
    r.packages.clear();
    r.checks.clear();
    assert!(r.validate().is_err());
    r.backend = Backend::ExternalFlake;
    assert!(r.validate().is_err());
    r.backend = Backend::Nixpkgs;
    assert!(r.validate().is_err());
    r.repository = "NixOS/nixpkgs".into();
    r.validate().unwrap();
}
#[test]
fn head_does_not_require_mergeability_and_preconditions_fail_closed() {
    let mut r = example();
    let (tested, p) = resolve_pr(&r, &pr(), None).unwrap();
    assert_eq!(tested, HEAD);
    assert!(p.merge.is_none());
    r.expected_head = Some(BASE.into());
    assert!(resolve_pr(&r, &pr(), None).is_err());
    r.expected_head = Some(HEAD.into());
    r.expected_base = Some(HEAD.into());
    assert!(resolve_pr(&r, &pr(), None).is_err());
}
#[test]
fn merge_requires_exact_ordered_parents() {
    let mut r = example();
    r.mode = Mode::Merge;
    assert!(resolve_pr(&r, &pr(), None).is_err());
    let mut p = pr();
    p["mergeable"] = json!(true);
    let mut c = json!({"sha":MERGE,"parents":[{"sha":BASE},{"sha":HEAD}]});
    assert_eq!(resolve_pr(&r, &p, Some(&c)).unwrap().0, MERGE);
    c["parents"] = json!([{"sha":HEAD},{"sha":BASE}]);
    assert!(resolve_pr(&r, &p, Some(&c)).is_err());
    c["parents"] = json!([{"sha":BASE},{"sha":MERGE}]);
    assert!(resolve_pr(&r, &p, Some(&c)).is_err());
}
#[test]
fn deterministic_hash_and_tamper_detection() {
    assert_eq!(
        canonical_digest(&json!({"a":1,"b":{"c":2,"d":3}})).unwrap(),
        canonical_digest(&json!({"b":{"d":3,"c":2},"a":1})).unwrap()
    );
    let mut p = plan();
    p.validate().unwrap();
    let original = p.digest.clone();
    p.seal().unwrap();
    assert_eq!(p.digest, original);
    p.target.commit = BASE.into();
    assert!(p.validate().is_err());
}
#[test]
fn absent_platform_or_check_is_never_success() {
    let p = plan();
    let r = report(&p);
    validate_result(&p, &r).unwrap();
    assert_eq!(aggregate(&p, vec![]).unwrap().build, Outcome::Failed);
    assert_eq!(
        aggregate(&p, vec![r.clone()]).unwrap().build,
        Outcome::Passed
    );
    let mut bad = r.clone();
    bad.tests = Outcome::NotRun;
    assert!(validate_result(&p, &bad).is_err());
    bad = r.clone();
    bad.effective.targets.pop();
    bad.effective.seal().unwrap();
    assert!(validate_result(&p, &bad).is_err());
    bad = r.clone();
    bad.runner_architecture = "aarch64".into();
    assert!(validate_result(&p, &bad).is_err());
    bad = r;
    bad.effective.system = "aarch64-linux".into();
    bad.effective.seal().unwrap();
    assert!(validate_result(&p, &bad).is_err());
}
#[test]
fn blocked_selection_does_not_pass_requested_checks() {
    let p = plan();
    let mut r = report(&p);
    r.build = Outcome::Blocked;
    r.tests = Outcome::NotRun;
    r.closure_export = Outcome::NotRun;
    r.effective.targets.clear();
    r.effective.seal().unwrap();
    r.target_outcomes.clear();
    r.test_evidence.clear();
    r.closure.clear();
    let aggregate = aggregate(&p, vec![r]).unwrap();
    assert_eq!(aggregate.tests, Outcome::Failed);
}
#[test]
fn publication_source_requires_successful_run_and_exact_attempt_jobs() {
    use repo_review::github::{validate_source_jobs, validate_source_run};
    let p = plan();
    let repo = &p.run_repository;
    let run = json!({"id":1,"repository":{"full_name":repo},"head_sha":HEAD,"path":WORKFLOW,"event":"workflow_dispatch","run_attempt":1,"status":"completed","conclusion":"success"});
    validate_source_run(repo, 1, 1, HEAD, &run).unwrap();
    for status in ["cancelled", "failure", "skipped", "timed_out", "neutral"] {
        let mut bad = run.clone();
        bad["conclusion"] = json!(status);
        assert!(validate_source_run(repo, 1, 1, HEAD, &bad).is_err());
    }
    let jobs:Vec<Value>=["controller","resolve","collect","build-x86_64-linux"].iter().map(|n|json!({"name":n,"run_id":1,"run_attempt":1,"head_sha":HEAD,"status":"completed","conclusion":"success"})).collect();
    validate_source_jobs(repo, 1, 1, HEAD, Some(&p), &jobs).unwrap();
    for status in ["cancelled", "failure", "skipped", "timed_out"] {
        let mut bad = jobs.clone();
        bad[3]["conclusion"] = json!(status);
        assert!(validate_source_jobs(repo, 1, 1, HEAD, Some(&p), &bad).is_err());
    }
    assert!(validate_source_jobs(repo, 1, 1, HEAD, Some(&p), &jobs[..3]).is_err());
    let mut bad = jobs.clone();
    bad[3]["run_attempt"] = json!(2);
    assert!(validate_source_jobs(repo, 1, 1, HEAD, Some(&p), &bad).is_err());
    bad = jobs.clone();
    bad[3]["name"] = json!("build-aarch64-linux");
    assert!(validate_source_jobs(repo, 1, 1, HEAD, Some(&p), &bad).is_err());
    bad = jobs.clone();
    bad.push(jobs[3].clone());
    assert!(validate_source_jobs(repo, 1, 1, HEAD, Some(&p), &bad).is_err());
}

#[test]
fn aggregate_claims_and_bundle_digests_cannot_be_forged() {
    let p = plan();
    let platform = report(&p);
    let mut r = aggregate(&p, vec![platform.clone()]).unwrap();
    r.bundle_digests
        .insert("x86_64-linux".into(), canonical_digest(&platform).unwrap());
    validate_review(&r).unwrap();
    let mut bad = r.clone();
    bad.tests = Outcome::NotRun;
    assert!(validate_review(&bad).is_err());
    bad = r.clone();
    bad.bundle_digests
        .insert("x86_64-linux".into(), "0".repeat(64));
    assert!(validate_review(&bad).is_err());
    bad = r;
    bad.missing_platforms.push("aarch64-linux".into());
    assert!(validate_review(&bad).is_err());
}

#[test]
fn export_failure_exits_unsuccessfully_and_retains_build_facts() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    let temp = tempfile::tempdir().unwrap();
    let p = plan();
    let plan_path = temp.path().join("plan.json");
    repo_review::write_json(&plan_path, &p).unwrap();
    let tools = temp.path().join("tools");
    std::fs::create_dir(&tools).unwrap();
    let lock = json!({"version":7,"root":"root","nodes":{"root":{}}});
    let metadata = json!({"locked":{"rev":HEAD,"narHash":"sha256-test"},"locks":lock});
    let drv = format!("{ROOT}.drv");
    let derivations = json!({&drv:{"system":"x86_64-linux","outputs":{"out":{"path":ROOT}}}});
    let closure = json!({ROOT:{"narHash":"sha256-test","narSize":1,"references":[]}});
    for (name, script) in [
        (
            "git",
            format!(
                "#!/bin/sh\ncase \"$1\" in\ninit) printf '{{}}' > flake.nix;;\nrev-parse) printf '%s' '{HEAD}';;\nesac\n"
            ),
        ),
        ("uname", "#!/bin/sh\nprintf x86_64".into()),
        (
            "nix",
            format!(
                "#!/bin/sh\ncase \"$1 $2 $3 $4\" in\n'config show system '*) printf x86_64-linux;;\n'config show sandbox '*) printf true;;\n--version*) printf 'Nix fixture';;\n'flake metadata '*) printf '%s' '{metadata}';;\neval*) printf '%s' '{drv}';;\n'derivation show '*) printf '%s' '{derivations}';;\nbuild*) printf '[]';;\n'path-info '*) printf '%s' '{closure}';;\ncopy*) printf 'intentional export failure' >&2; exit 1;;\n*) exit 9;;\nesac\n"
            ),
        ),
    ] {
        let path = tools.join(name);
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let out = temp.path().join("bundle");
    let result = Command::new(env!("CARGO_BIN_EXE_repo-review"))
        .args(["build", "--plan"])
        .arg(&plan_path)
        .args(["--system", "x86_64-linux", "--output"])
        .arg(&out)
        .env("PATH", &tools)
        .output()
        .unwrap();
    let r: PlatformResult = repo_review::read_json(&out.join("review-result.json")).unwrap();
    assert_eq!(r.build, Outcome::Passed, "{:?}", r.error);
    assert_eq!(r.tests, Outcome::Passed);
    assert_eq!(r.closure_export, Outcome::Failed);
    assert!(r.error.as_deref().is_some_and(|s| s.contains("nix failed")));
    assert!(
        !result.status.success(),
        "export failure must not return exit code zero"
    );
    validate_bundle(&p, &r, &out).unwrap();
    assert!(!aggregate(&p, vec![r]).unwrap().successful());
}
#[test]
fn local_no_changes_requires_complete_nixpkgs_evidence() {
    let mut p = plan();
    p.request.backend = Backend::Nixpkgs;
    p.request.repository = "NixOS/nixpkgs".into();
    p.request.packages.clear();
    p.request.checks.clear();
    p.target.repository = p.request.repository.clone();
    p.pr.as_mut().unwrap().url = "https://github.com/NixOS/nixpkgs/pull/1".into();
    p.request_id = canonical_digest(&p.request).unwrap();
    p.seal().unwrap();
    let mut r = report(&p);
    r.effective.metadata_digest = p.digest.clone();
    r.effective.targets.clear();
    r.effective.flake_reference = None;
    r.effective.lock = None;
    r.effective.lock_digest = None;
    r.effective.source_nar_hash = None;
    r.effective.seal().unwrap();
    r.target_outcomes.clear();
    r.test_evidence.clear();
    r.closure.clear();
    r.build = Outcome::NoChanges;
    r.tests = Outcome::NotRun;
    r.closure_export = Outcome::NotRun;
    let temp = tempfile::tempdir().unwrap();
    repo_review::write_json(&temp.path().join("plan.json"), &p).unwrap();
    repo_review::write_json(&temp.path().join("effective-plan.json"), &r.effective).unwrap();
    r.files = inventory(temp.path()).unwrap();
    assert!(validate_bundle(&p, &r, temp.path()).is_err());
    let selection = json!({"schema_version":1,"backend_version":"3.7.0","tested_commit":HEAD,"base_commit":BASE,"system":"x86_64-linux","changed_attributes":[],"derivations":[]});
    let path = temp.path().join("nixpkgs-selection.json");
    repo_review::write_json(&path, &selection).unwrap();
    r.files = inventory(temp.path()).unwrap();
    validate_bundle(&p, &r, temp.path()).unwrap();
    assert!(r.successful());
    for (key, value) in [
        ("tested_commit", json!(MERGE)),
        ("changed_attributes", json!(["hello"])),
        ("derivations", json!([{}])),
    ] {
        let mut bad = selection.clone();
        bad[key] = value;
        repo_review::write_json(&path, &bad).unwrap();
        r.files = inventory(temp.path()).unwrap();
        assert!(validate_bundle(&p, &r, temp.path()).is_err());
    }
}
#[test]
fn closure_requires_all_runtime_dependencies_and_no_extras() {
    let mut c = BTreeMap::from([(
        ROOT.into(),
        Nar {
            nar_hash: "sha256-test".into(),
            nar_size: 1,
            references: vec![DEP.into()],
        },
    )]);
    assert!(validate_closure(&[ROOT.into()], &c).is_err());
    c.insert(
        DEP.into(),
        Nar {
            nar_hash: "sha256-test".into(),
            nar_size: 1,
            references: vec![],
        },
    );
    validate_closure(&[ROOT.into()], &c).unwrap();
    c.get_mut(ROOT).unwrap().references.clear();
    assert!(validate_closure(&[ROOT.into()], &c).is_err());
}
#[test]
fn unsafe_artifact_symlinks_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink("/etc/passwd", temp.path().join("log")).unwrap();
    assert!(inventory(temp.path()).is_err());
}
#[test]
fn publication_requires_request_and_exact_both_digests() {
    let mut p = plan();
    let r = report(&p);
    let bundle = "0".repeat(64);
    assert!(authorize_publication(&p, &r, &r.effective.digest, &bundle, &bundle).is_err());
    p.request.publication = Publication::RequestApproval;
    p.request.cache_profile = Some("attic-existing".into());
    p.request_id = canonical_digest(&p.request).unwrap();
    p.seal().unwrap();
    let r = report(&p);
    authorize_publication(&p, &r, &r.effective.digest, &bundle, &bundle).unwrap();
    assert!(authorize_publication(&p, &r, &r.effective.digest, &bundle, &"1".repeat(64)).is_err());
    assert!(authorize_publication(&p, &r, &"1".repeat(64), &bundle, &bundle).is_err());
}
#[test]
fn command_arguments_remain_separate_and_secrets_scrubbed() {
    let a = process::args(&["build", "name with spaces;$(id)"]);
    let c = process::command("nix", &a, None);
    assert_eq!(
        c.get_args().collect::<Vec<_>>(),
        vec!["build", "name with spaces;$(id)"]
    );
    assert!(c.get_envs().any(|(k, v)| k == "ATTIC_TOKEN" && v.is_none()));
    let a = process::nix_args(&[
        "copy",
        "--from",
        "https://cache.example",
        "--to",
        "local?root=/tmp/new",
        ROOT,
    ]);
    assert!(!a.contains(&"build".into()));
    assert!(!a.contains(&"--no-check-sigs".into()));
}
#[test]
fn public_retrieval_enforces_signatures_without_building() {
    let a = retrieval_args(
        "https://cache.example",
        &["review:KEY".into()],
        std::path::Path::new("/fresh"),
        false,
    );
    assert!(a.iter().any(|v| v == "local?root=/fresh&require-sigs=true"));
    assert!(
        a.windows(3)
            .any(|v| v == ["--option", "require-sigs", "true"])
    );
    assert!(
        a.windows(3)
            .any(|v| v == ["--option", "extra-trusted-public-keys", "review:KEY"])
    );
    assert!(!a.iter().any(|v| {
        [
            "build",
            "eval",
            "realise",
            "--no-check-sigs",
            "trusted-public-keys",
        ]
        .contains(&v.as_str())
    }));
}

#[test]
fn attic_uses_private_xdg_configuration_and_supported_argv() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("config");
    let c = CacheProfile {
        kind: "attic".into(),
        server: Some("https://attic.example/".into()),
        cache: "existing-cache".into(),
        url: "https://attic.example/existing-cache".into(),
        public_keys: vec![],
    };
    let cmd = attic_command(&c, "TEST_PRIVATE_TOKEN", &home, &[ROOT.into()]).unwrap();
    assert_eq!(
        cmd.get_args().collect::<Vec<_>>(),
        vec!["push", "review:existing-cache", ROOT]
    );
    assert!(
        cmd.get_envs()
            .any(|(k, v)| k == "XDG_CONFIG_HOME" && v == Some(home.as_os_str()))
    );
    assert!(!format!("{cmd:?}").contains("TEST_PRIVATE_TOKEN"));
    let config = home.join("attic/config.toml");
    assert_eq!(
        std::fs::metadata(&config).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(
        std::fs::read_to_string(config)
            .unwrap()
            .contains("token = \"TEST_PRIVATE_TOKEN\"")
    );
}
#[test]
fn external_lock_requires_direct_nonflake_exact_source() {
    let mut p = plan();
    p.request.backend = Backend::ExternalFlake;
    p.request.recipe = Some(Recipe {
        repository: "trusted/recipe".into(),
        commit: BASE.into(),
        directory: ".".into(),
        source_input: "source".into(),
    });
    let mut lock = json!({"version":7,"root":"root","nodes":{"root":{"inputs":{"source":"source"}},"source":{"flake":false,"locked":{"type":"github","owner":"contributor","repo":"project","rev":HEAD,"narHash":"sha256-test"}}}});
    validate_lock(&p, &lock).unwrap();
    lock["nodes"]["source"]["flake"] = json!(true);
    assert!(validate_lock(&p, &lock).is_err());
    lock["nodes"]["source"]["flake"] = json!(false);
    lock["nodes"]["source"]["locked"]["rev"] = json!(BASE);
    assert!(validate_lock(&p, &lock).is_err());
}
