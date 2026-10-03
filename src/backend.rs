//! Unprivileged execution only. Never call these functions from publication/report jobs.
use crate::{
    artifact, canonical_digest,
    contract::*,
    github,
    process::{args, nix_args, run},
    read_json, write_json,
};
use anyhow::{Result, bail, ensure};
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};

pub fn source_ref(plan: &Plan) -> String {
    let repo = if plan.request.mode == Mode::Head {
        plan.pr
            .as_ref()
            .map(|p| p.head_repository.as_str())
            .unwrap_or(&plan.target.repository)
    } else {
        &plan.target.repository
    };
    format!("github:{repo}/{}", plan.target.commit)
}
pub fn flake_ref(plan: &Plan) -> String {
    if let Some(r) = &plan.request.recipe {
        format!("github:{}/{}?dir={}", r.repository, r.commit, r.directory)
    } else {
        format!("{}?dir={}", source_ref(plan), plan.request.directory)
    }
}
pub fn validate_lock(plan: &Plan, lock: &Value) -> Result<Option<String>> {
    let nodes = lock["nodes"]
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("missing lock nodes"))?;
    ensure!(
        lock["version"] == 7 && lock["root"].as_str().is_some(),
        "unsupported lock format"
    );
    for (n, v) in nodes {
        if n == lock["root"].as_str().unwrap_or("") {
            continue;
        }
        let l = &v["locked"];
        ensure!(
            l.is_object() && l["narHash"].as_str().is_some(),
            "unlocked input"
        );
        // No local path, mutable registry, token-bearing URL, or unrecognized transport.
        match l["type"].as_str() {
            Some("github") => {
                repository(&format!(
                    "{}/{}",
                    l["owner"].as_str().unwrap_or(""),
                    l["repo"].as_str().unwrap_or("")
                ))?;
                sha(l["rev"].as_str().unwrap_or(""))?;
            }
            Some("git" | "tarball" | "file") => {
                artifact::public_url(l["url"].as_str().unwrap_or(""))?
            }
            _ => bail!("unsupported effective-lock transport"),
        }
    }
    if let Some(r) = &plan.request.recipe {
        let root = lock["root"].as_str().unwrap_or("");
        let node = lock["nodes"][root]["inputs"][&r.source_input]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("source input must be a direct non-flake input"))?;
        let input = &lock["nodes"][node];
        let source_repo = source_ref(plan);
        let expected = source_repo
            .strip_prefix("github:")
            .unwrap_or("")
            .rsplit_once('/')
            .ok_or_else(|| anyhow::anyhow!("invalid source"))?
            .0;
        ensure!(
            input["flake"] == false
                && input["locked"]["type"] == "github"
                && input["locked"]["rev"] == plan.target.commit
                && format!(
                    "{}/{}",
                    input["locked"]["owner"].as_str().unwrap_or(""),
                    input["locked"]["repo"].as_str().unwrap_or("")
                ) == expected,
            "effective source input is not exact flake=false target"
        );
        return Ok(input["locked"]["narHash"].as_str().map(str::to_owned));
    }
    Ok(None)
}
pub fn prepare(plan: &Plan, system: &str, out: &Path) -> Result<EffectivePlan> {
    let verified = tempfile::tempdir()?;
    github::checkout(&plan.target, &verified.path().join("target"))?;
    let recipe_tree = if let Some(i) = &plan.recipe {
        let path = verified.path().join("recipe");
        github::checkout(i, &path)?;
        path
    } else {
        verified.path().join("target")
    };
    let directory = plan
        .request
        .recipe
        .as_ref()
        .map(|r| r.directory.as_str())
        .unwrap_or(&plan.request.directory);
    let flake = recipe_tree.join(directory).join("flake.nix");
    ensure!(
        flake.is_file()
            && flake
                .canonicalize()?
                .starts_with(recipe_tree.canonicalize()?),
        "unsupported: repository/directory has no regular in-tree flake.nix"
    );
    let reference = flake_ref(plan);
    let lock_path = out.join("effective.lock");
    let mut a = nix_args(&[
        "flake",
        "metadata",
        "--json",
        "--no-write-lock-file",
        &reference,
    ]);
    // First metadata invocation verifies the recipe's declared non-flake input before overriding it.
    let initial: Value =
        serde_json::from_str(&run("nix", &a, None, Some(&out.join("metadata.log")))?)?;
    if let Some(r) = &plan.request.recipe {
        let lock = &initial["locks"];
        let root = lock["root"].as_str().unwrap_or("");
        let node = lock["nodes"][root]["inputs"][&r.source_input]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("recipe source input absent"))?;
        ensure!(
            lock["nodes"][node]["flake"] == false,
            "recipe input must have flake=false"
        );
        a.extend(args(&[
            "--override-input",
            &r.source_input,
            &source_ref(plan),
        ]));
    }
    let metadata: Value =
        serde_json::from_str(&run("nix", &a, None, Some(&out.join("lock.log")))?)?;
    ensure!(
        metadata["locked"]["rev"] == plan.recipe.as_ref().unwrap_or(&plan.target).commit,
        "Nix fetched a different root revision"
    );
    let lock = metadata["locks"].clone();
    let source_nar_hash = validate_lock(plan, &lock)?
        .or_else(|| metadata["locked"]["narHash"].as_str().map(str::to_owned));
    write_json(&lock_path, &lock)?;
    let mut targets = vec![];
    for (kind, names) in [
        ("packages", &plan.request.packages),
        ("checks", &plan.request.checks),
    ] {
        for name in names {
            let installable = format!("{reference}#{kind}.{system}.{name}");
            let mut eval = nix_args(&[
                "eval",
                "--raw",
                "--no-update-lock-file",
                "--reference-lock-file",
                lock_path.to_str().unwrap_or(""),
                &format!("{installable}.drvPath"),
            ]);
            if let Some(r) = &plan.request.recipe {
                eval.extend(args(&[
                    "--override-input",
                    &r.source_input,
                    &source_ref(plan),
                ]));
            }
            let drv = run(
                "nix",
                &eval,
                None,
                Some(&out.join(format!("eval-{kind}-{name}.log"))),
            )?;
            artifact::store_path(&drv)?;
            ensure!(drv.ends_with(".drv"), "not a derivation");
            let d: Value = serde_json::from_str(&run(
                "nix",
                &nix_args(&["derivation", "show", &drv]),
                None,
                None,
            )?)?;
            let item = d
                .get(&drv)
                .or_else(|| d.get("derivations").and_then(|v| v.get(&drv)))
                .ok_or_else(|| anyhow::anyhow!("missing derivation"))?;
            ensure!(item["system"] == system, "derivation platform mismatch");
            let outputs: BTreeMap<String, String> = item["outputs"]
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("missing outputs"))?
                .iter()
                .map(|(k, v)| {
                    let p = v["path"].as_str().ok_or_else(|| {
                        anyhow::anyhow!(
                            "dynamic/content-addressed outputs unsupported by frozen-path contract"
                        )
                    })?;
                    artifact::store_path(p)?;
                    Ok((k.clone(), p.into()))
                })
                .collect::<Result<_>>()?;
            ensure!(!outputs.is_empty(), "empty derivation outputs");
            targets.push(Target {
                installable,
                derivation: drv,
                outputs,
                check: kind == "checks",
            });
        }
    }
    let mut e = EffectivePlan {
        metadata_digest: plan.digest.clone(),
        system: system.into(),
        flake_reference: Some(reference),
        lock_digest: Some(canonical_digest(&lock)?),
        lock: Some(lock),
        source_nar_hash,
        targets,
        digest: String::new(),
    };
    e.seal()?;
    write_json(&out.join("effective-plan.json"), &e)?;
    Ok(e)
}

pub fn build(plan: &Plan, system: &str, out: &Path) -> Result<PlatformResult> {
    plan.validate()?;
    ensure!(
        plan.request.systems.iter().any(|s| s == system),
        "unrequested platform"
    );
    ensure!(!out.exists(), "output directory must be new");
    fs::create_dir_all(out)?;
    let mut result = PlatformResult {
        schema_version: VERSION,
        plan: plan.clone(),
        effective: EffectivePlan {
            metadata_digest: plan.digest.clone(),
            system: system.into(),
            flake_reference: None,
            lock: None,
            lock_digest: None,
            source_nar_hash: None,
            targets: vec![],
            digest: String::new(),
        },
        runner_architecture: String::new(),
        nix_version: String::new(),
        sandbox: String::new(),
        build: Outcome::NotRun,
        tests: Outcome::NotRun,
        closure_export: Outcome::NotRun,
        publication: Outcome::NotRun,
        retrieval: Outcome::NotRun,
        error: None,
        files: BTreeMap::new(),
        closure: BTreeMap::new(),
        target_outcomes: BTreeMap::new(),
        test_evidence: BTreeMap::new(),
    };
    let execution = (|| -> Result<()> {
        result.runner_architecture = run("uname", &args(&["-m"]), None, None)?.trim().into();
        let (_, arch, sandbox) = runner(system)?;
        ensure!(
            result.runner_architecture == arch,
            "runner architecture mismatch"
        );
        let native = run("nix", &args(&["config", "show", "system"]), None, None)?;
        ensure!(native.trim() == system, "Nix platform mismatch");
        result.nix_version = run("nix", &args(&["--version"]), None, None)?.trim().into();
        result.sandbox = run("nix", &args(&["config", "show", "sandbox"]), None, None)?
            .trim()
            .into();
        ensure!(result.sandbox == sandbox, "sandbox policy mismatch");
        result.effective = if plan.request.backend == Backend::Nixpkgs {
            nixpkgs_prepare(plan, system, out)?
        } else {
            prepare(plan, system, out)?
        };
        if result.effective.targets.is_empty() {
            ensure!(plan.request.backend == Backend::Nixpkgs, "absent targets");
            result.build = Outcome::NoChanges;
            return Ok(());
        }
        result.build = Outcome::Passed;
        for (index, t) in result.effective.targets.iter().enumerate() {
            let selected = format!("{}^*", t.derivation);
            let log = out.join(format!("build-{index}.log"));
            let success = run(
                "nix",
                &nix_args(&["build", "--no-link", "--json", "-L", &selected]),
                None,
                Some(&log),
            )
            .is_ok();
            let mut outcome = if success {
                Outcome::Passed
            } else {
                Outcome::Failed
            };
            if t.check && success {
                if plan.request.test_profile == "checks-rebuild-v1" {
                    // Rebuild only the selected check, never unrelated dependencies.
                    if run(
                        "nix",
                        &nix_args(&["build", "--no-link", "--rebuild", "-L", &selected]),
                        None,
                        Some(&out.join(format!("test-{index}.log"))),
                    )
                    .is_err()
                    {
                        outcome = Outcome::Failed;
                    }
                    result
                        .test_evidence
                        .insert(t.installable.clone(), "explicit-check-rebuild".into());
                } else {
                    result.test_evidence.insert(
                        t.installable.clone(),
                        "realized; execution-versus-substitution-not-proven".into(),
                    );
                }
            }
            if outcome != Outcome::Passed {
                result.build = Outcome::Failed;
            }
            result
                .target_outcomes
                .insert(t.installable.clone(), outcome);
        }
        result.tests = if !result.effective.targets.iter().any(|t| t.check) {
            Outcome::NotRun
        } else if result
            .effective
            .targets
            .iter()
            .filter(|t| t.check)
            .all(|t| result.target_outcomes.get(&t.installable) == Some(&Outcome::Passed))
        {
            Outcome::Passed
        } else {
            Outcome::Failed
        };
        ensure!(
            result.build == Outcome::Passed,
            "one or more requested targets failed"
        );
        result.closure_export = Outcome::Failed;
        artifact::export(&mut result, out)?;
        result.closure_export = Outcome::Passed;
        Ok(())
    })();
    if let Err(e) = execution {
        if result.build == Outcome::NotRun {
            result.build = if e.to_string().starts_with("unsupported:") {
                Outcome::Unsupported
            } else {
                Outcome::Blocked
            };
        }
        result.error = Some(e.to_string());
    }
    if result.effective.digest.is_empty() {
        result.effective.seal()?;
    }
    result.files = artifact::inventory(out)?;
    write_json(&out.join("review-result.json"), &result)?;
    Ok(result)
}

fn nixpkgs_prepare(plan: &Plan, system: &str, out: &Path) -> Result<EffectivePlan> {
    let pr = plan
        .pr
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("missing PR"))?;
    let checkout = tempfile::tempdir()?;
    let target = checkout.path().join("nixpkgs");
    github::checkout(&plan.target, &target)?;
    run(
        "git",
        &args(&[
            "fetch",
            "--depth=1",
            &format!("https://github.com/{}.git", plan.target.repository),
            &pr.base,
        ]),
        Some(&target),
        None,
    )?;
    let plan_path = out.join("metadata-plan.json");
    write_json(&plan_path, plan)?;
    let selected_path = out.join("nixpkgs-selection.json");
    run(
        "repo-review-nixpkgs-select",
        &args(&[
            plan_path.to_str().unwrap_or(""),
            system,
            selected_path.to_str().unwrap_or(""),
        ]),
        Some(&target),
        Some(&out.join("nixpkgs-selection.log")),
    )?;
    let selection: Value = read_json(&selected_path)?;
    ensure!(
        selection["tested_commit"] == plan.target.commit
            && selection["base_commit"] == pr.base
            && selection["system"] == system
            && selection["backend_version"] == "3.7.0",
        "Nixpkgs selection identity mismatch"
    );
    let selected = selection["derivations"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("missing Nixpkgs selection"))?;
    let mut targets = vec![];
    for s in selected {
        let drv = s["derivation"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing derivation"))?;
        artifact::store_path(drv)?;
        let d: Value = serde_json::from_str(&run(
            "nix",
            &nix_args(&["derivation", "show", drv]),
            None,
            None,
        )?)?;
        let item = d
            .get(drv)
            .or_else(|| d.get("derivations").and_then(|v| v.get(drv)))
            .ok_or_else(|| anyhow::anyhow!("missing derivation"))?;
        ensure!(
            item["system"] == system,
            "wrong Nixpkgs derivation platform"
        );
        let outputs = item["outputs"]
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("missing outputs"))?
            .iter()
            .map(|(name, v)| {
                let path = v["path"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("unfrozen output"))?;
                artifact::store_path(path)?;
                Ok((name.clone(), path.into()))
            })
            .collect::<Result<_>>()?;
        let attribute = s["attribute"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing attribute"))?;
        ensure!(
            attribute.len() < 256
                && attribute
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
            "unsupported Nixpkgs attribute"
        );
        targets.push(Target {
            installable: format!("nixpkgs:{}/{system}/{attribute}", plan.target.commit),
            derivation: drv.into(),
            outputs,
            check: s["check"] == true,
        });
    }
    let mut e = EffectivePlan {
        metadata_digest: plan.digest.clone(),
        system: system.into(),
        flake_reference: None,
        lock: None,
        lock_digest: None,
        source_nar_hash: None,
        targets,
        digest: String::new(),
    };
    e.seal()?;
    write_json(&out.join("effective-plan.json"), &e)?;
    Ok(e)
}
