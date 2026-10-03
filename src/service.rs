use crate::{
    artifact, canonical_digest,
    contract::*,
    github,
    process::{args, command, run, run_command},
    read_json, write_json,
};
use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewResult {
    pub schema_version: u32,
    pub plan: Plan,
    pub build: Outcome,
    pub tests: Outcome,
    pub closure_export: Outcome,
    pub publication: Outcome,
    pub retrieval: Outcome,
    pub platforms: BTreeMap<String, PlatformResult>,
    pub missing_platforms: Vec<String>,
    pub bundle_digests: BTreeMap<String, String>,
    pub provenance: String,
}
impl ReviewResult {
    pub fn successful(&self) -> bool {
        self.missing_platforms.is_empty()
            && self.platforms.len() == self.plan.request.systems.len()
            && self.platforms.values().all(PlatformResult::successful)
            && matches!(self.tests, Outcome::Passed | Outcome::NotRun)
    }
}
pub fn aggregate(plan: &Plan, reports: Vec<PlatformResult>) -> Result<ReviewResult> {
    plan.validate()?;
    let mut platforms = BTreeMap::new();
    for r in reports {
        artifact::validate_result(plan, &r)?;
        ensure!(
            platforms.insert(r.effective.system.clone(), r).is_none(),
            "duplicate platform report"
        );
    }
    let missing: Vec<_> = plan
        .request
        .systems
        .iter()
        .filter(|s| !platforms.contains_key(*s))
        .cloned()
        .collect();
    let passed = missing.is_empty()
        && platforms
            .values()
            .all(|r| matches!(r.build, Outcome::Passed | Outcome::NoChanges));
    let required_checks = !plan.request.checks.is_empty()
        || platforms
            .values()
            .any(|r| r.effective.targets.iter().any(|t| t.check));
    let tests = if !required_checks {
        Outcome::NotRun
    } else if missing.is_empty()
        && platforms.values().all(|r| {
            if plan.request.checks.is_empty() {
                !r.effective.targets.iter().any(|t| t.check) || r.tests == Outcome::Passed
            } else {
                r.tests == Outcome::Passed
            }
        })
    {
        Outcome::Passed
    } else {
        Outcome::Failed
    };
    let closure_export = if !missing.is_empty()
        || platforms
            .values()
            .any(|r| r.closure_export == Outcome::Failed)
    {
        Outcome::Failed
    } else if platforms
        .values()
        .any(|r| r.closure_export == Outcome::Passed)
    {
        if platforms
            .values()
            .all(|r| r.closure_export == Outcome::Passed || r.build == Outcome::NoChanges)
        {
            Outcome::Passed
        } else {
            Outcome::Failed
        }
    } else {
        Outcome::NotRun
    };
    Ok(ReviewResult { schema_version: VERSION, plan: plan.clone(), build: if passed { Outcome::Passed } else { Outcome::Failed }, tests, closure_export,
        publication: if plan.request.publication == Publication::RequestApproval { Outcome::Blocked } else { Outcome::NotRun }, retrieval: Outcome::NotRun,
        platforms, missing_platforms: missing, bundle_digests: BTreeMap::new(),
        provenance: "GitHub metadata is frozen by resolver; effective locks, derivations, and test evidence are untrusted runner observations, not independent attestations.".into() })
}
pub fn collect(plan: &Plan, inputs: &Path, out: &Path) -> Result<ReviewResult> {
    fs::create_dir_all(out)?;
    let mut reports = vec![];
    let mut bundles = BTreeMap::new();
    for system in &plan.request.systems {
        let dir = inputs.join(format!(
            "platform-{system}-{}-{}",
            plan.run_id, plan.run_attempt
        ));
        if !dir.exists() {
            continue;
        }
        let r: PlatformResult = read_json(&dir.join("review-result.json"))?;
        ensure!(
            r.effective.system == *system,
            "artifact platform/name mismatch"
        );
        let digest = artifact::validate_bundle(plan, &r, &dir)?;
        bundles.insert(system.clone(), digest);
        reports.push(r);
    }
    let mut result = aggregate(plan, reports)?;
    result.bundle_digests = bundles;
    write_json(&out.join("review-result.json"), &result)?;
    fs::write(out.join("report.md"), markdown(&result))?;
    fs::write(out.join("consume.md"), consume(&result))?;
    Ok(result)
}
pub fn markdown(r: &ReviewResult) -> String {
    let p = &r.plan;
    let mut text = format!(
        "<!-- repo-review:{}:{} -->\n# Repository review\n\nTarget: `{}` at `{}` (tree `{}`).\n\nController: `{}` at `{}`.\n\nMetadata plan: `{}`.\n\nRun: https://github.com/{}/actions/runs/{}/attempts/{}\n\nBuild: **{:?}**; tests: **{:?}**; closure export: **{:?}**; publication: **{:?}**; fresh-store retrieval: **{:?}**.\n\n",
        p.target.repository,
        p.target.commit,
        p.target.repository,
        p.target.commit,
        p.target.tree,
        p.controller.repository,
        p.controller.commit,
        p.digest,
        p.run_repository,
        p.run_id,
        p.run_attempt,
        r.build,
        r.tests,
        r.closure_export,
        r.publication,
        r.retrieval
    );
    if let Some(recipe) = &p.recipe {
        text.push_str(&format!(
            "Recipe: `{}` at `{}`.\n\n",
            recipe.repository, recipe.commit
        ));
    }
    if let Some(pr) = &p.pr {
        text.push_str(&format!(
            "PR: {}. Head: `{}` from `{}`; base: `{}`; mode: `{:?}`.\n\n",
            pr.url, pr.head, pr.head_repository, pr.base, p.request.mode
        ));
    }
    for (system, platform) in &r.platforms {
        text.push_str(&format!(
            "- `{system}`: build **{:?}**, tests **{:?}**; effective plan `{}`; bundle `{}`.\n",
            platform.build,
            platform.tests,
            platform.effective.digest,
            r.bundle_digests
                .get(system)
                .map(String::as_str)
                .unwrap_or("unverified")
        ));
    }
    for system in &r.missing_platforms {
        text.push_str(&format!("- `{system}`: **FAILED — missing report**.\n"));
    }
    text.push_str("\nNo approval or merge authority is granted by this report. Build-reported facts are not independent attestations. See artifacts for logs, effective locks, runtime closures, and consumption instructions.\n");
    text
}
pub fn consume(r: &ReviewResult) -> String {
    let mut text = String::from(
        "# Consume exact review outputs\n\nPublication and cache retrieval are separate gates. A local closure transfer does not establish remote-cache availability.\n\n## Fetch without source building\n\nAfter approved publication, use `repo-review fetch --result PLATFORM/review-result.json --policy TRUSTED-CONTROLLER/policy.json --profile PROFILE --destination NEW-EMPTY-STORE`. This performs only `nix copy` and `nix path-info` on exact store paths; it never evaluates or builds a target.\n\nDo not use `--no-check-sigs`. Keep the standard trusted keys and append only the reviewed profile's public keys. Adding a cache key trusts its operator to provide executable software; a signature says nothing about test quality.\n\n## Optional output comparison\n\nIn a disposable unprivileged environment, evaluate the pinned flake with `--reference-lock-file effective.lock --no-update-lock-file --option allow-import-from-derivation false --option accept-flake-config false`, compare every derivation/output path against `effective-plan.json`, then copy the verified exact paths. Never use a build command as a retrieval check: `--max-jobs 0 --builders ''` still permits preferLocalBuild derivations.\n\n",
    );
    for (system, p) in &r.platforms {
        text.push_str(&format!("## {system}\n\nPinned recipe: `{}`\n\nEffective lock digest: `{}`. Retain the exact `effective.lock` artifact; the original recipe is immutable. A downstream recipe may commit that lock.\n\nExact roots:\n\n```text\n", p.effective.flake_reference.as_deref().unwrap_or("legacy Nixpkgs report only"), p.effective.lock_digest.as_deref().unwrap_or("none")));
        for root in artifact::roots(p) {
            text.push_str(&format!("{root}\n"));
        }
        text.push_str("```\n\n");
    }
    text.push_str("## Stage a configuration change\n\nReview a NixOS/Home Manager change pinning the recipe commit and effective lock, verify its expected output identities, and stage it in your own configuration repository. This CLI never edits that configuration.\n\n## Activate separately\n\nActivation is a separate, explicitly user-authorized deployment operation. Fetching or passing this review does not authorize activation.\n");
    text
}
pub fn post(r: &ReviewResult, out: &Path) -> Result<Value> {
    r.plan.validate()?;
    ensure!(r.plan.request.post_result, "posting not requested");
    let pr = r.plan.pr.as_ref().ok_or_else(|| anyhow::anyhow!("no PR"))?;
    fs::write(out, markdown(r))?;
    ensure!(fs::metadata(out)?.len() < 60000, "comment too large");
    if std::env::var("GH_TOKEN").unwrap_or_default().is_empty() {
        return Ok(
            json!({"posted":false,"outcome":"blocked","manual_argv":["gh","pr","comment",pr.number.to_string(),"--repo",r.plan.target.repository,"--body-file",out]}),
        );
    }
    // Explicit report-only credential. No automatic fallback to GITHUB_TOKEN.
    let current = github::api(&format!(
        "repos/{}/pulls/{}",
        r.plan.target.repository, pr.number
    ))?;
    ensure!(
        current["head"]["sha"] == pr.head && current["base"]["sha"] == pr.base,
        "PR changed: retain report instead of posting stale comment"
    );
    run(
        "gh",
        &args(&[
            "pr",
            "comment",
            &pr.number.to_string(),
            "--repo",
            &r.plan.target.repository,
            "--body-file",
            out.to_str().unwrap_or(""),
        ]),
        None,
        None,
    )?;
    Ok(json!({"posted":true,"outcome":"passed"}))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheProfile {
    pub kind: String,
    pub url: String,
    pub public_keys: Vec<String>,
    pub cache: String,
    pub server: Option<String>,
}
pub fn cache_profile(policy: &Path, profile: &str) -> Result<CacheProfile> {
    ensure!(
        ["attic-existing", "cachix-existing"].contains(&profile),
        "unknown profile"
    );
    let p: Value = read_json(policy)?;
    let c: CacheProfile = serde_json::from_value(p["caches"][profile].clone())?;
    artifact::public_url(&c.url)?;
    ensure!(
        name(&c.cache) && !c.public_keys.is_empty(),
        "incomplete approved cache configuration"
    );
    for k in &c.public_keys {
        ensure!(
            k.len() < 200
                && k.split_once(':').is_some_and(|(n, v)| !n.is_empty()
                    && v.len() == 44
                    && v.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"/+= ".contains(&b)))
                && !k.contains(' '),
            "invalid public key"
        );
    }
    if let Some(s) = &c.server {
        artifact::public_url(s)?;
    }
    ensure!(
        (profile == "attic-existing" && c.kind == "attic" && c.server.is_some())
            || (profile == "cachix-existing" && c.kind == "cachix"),
        "cache kind mismatch"
    );
    Ok(c)
}
pub fn authorize_publication(
    plan: &Plan,
    r: &PlatformResult,
    plan_digest: &str,
    bundle_digest: &str,
    actual_bundle: &str,
) -> Result<()> {
    hash(plan_digest)?;
    hash(bundle_digest)?;
    artifact::validate_result(plan, r)?;
    ensure!(
        plan.request.publication == Publication::RequestApproval
            && plan.request.cache_profile.is_some(),
        "publication not requested"
    );
    ensure!(
        r.successful() && r.build == Outcome::Passed,
        "publication requires passing result with frozen outputs"
    );
    ensure!(
        r.effective.digest == plan_digest && actual_bundle == bundle_digest,
        "approval digest mismatch"
    );
    Ok(())
}
pub fn publish(
    plan: &Plan,
    r: &PlatformResult,
    bundle: &Path,
    policy: &Path,
    approved_plan: &str,
    approved_bundle: &str,
) -> Result<Value> {
    let digest = artifact::validate_bundle(plan, r, bundle)?;
    authorize_publication(plan, r, approved_plan, approved_bundle, &digest)?;
    let p: Value = read_json(policy)?;
    ensure!(
        p["publication_enabled"] == true,
        "publication disabled by trusted policy"
    );
    let c = cache_profile(policy, plan.request.cache_profile.as_deref().unwrap_or(""))?;
    // The workflow has already authenticated the exact source run and dispatch actor.
    // This fresh job imports unsigned review data only into its disposable store.
    let cache_url = format!("file://{}", bundle.join("cache").canonicalize()?.display());
    let isolated = tempfile::tempdir()?;
    artifact::retrieve(r, &cache_url, &[], &isolated.path().join("store"), true)?;
    let mut import = crate::process::nix_args(&["copy", "--from", &cache_url, "--no-check-sigs"]);
    import.extend(artifact::roots(r));
    run("nix", &import, None, None)?;
    ensure!(
        artifact::closure_info(&artifact::roots(r), None)? == r.closure,
        "imported closure mismatch"
    );
    if c.kind == "attic" {
        ensure!(
            std::env::var("ATTIC_SERVER").ok().as_ref() == c.server.as_ref()
                && std::env::var("ATTIC_CACHE").ok().as_deref() == Some(&c.cache),
            "Attic profile/config drift"
        );
        let token = std::env::var("ATTIC_TOKEN")?;
        ensure!(!token.is_empty(), "missing Attic credentials");
        run_command(
            attic_command(
                &c,
                &token,
                &isolated.path().join("config"),
                &artifact::roots(r),
            )?,
            None,
        )?;
    } else {
        ensure!(
            std::env::var("CACHIX_CACHE").ok().as_deref() == Some(&c.cache),
            "Cachix profile/config drift"
        );
        let token = std::env::var("CACHIX_AUTH_TOKEN")?;
        ensure!(!token.is_empty(), "missing Cachix credentials");
        let mut a = args(&["push", &c.cache]);
        a.extend(artifact::roots(r));
        let mut cmd = crate::process::command("cachix", &a, None);
        cmd.env("CACHIX_AUTH_TOKEN", token);
        if let Ok(k) = std::env::var("CACHIX_SIGNING_KEY")
            && !k.is_empty()
        {
            cmd.env("CACHIX_SIGNING_KEY", k);
        }
        run_command(cmd, None)?;
    }
    Ok(
        json!({"schema_version":VERSION,"publication":"passed","retrieval":"not_run","effective_plan_digest":approved_plan,"bundle_digest":approved_bundle,"cache_url":c.url,"public_keys":c.public_keys}),
    )
}

/// The pinned Attic reads only XDG_CONFIG_HOME/attic/config.toml; no --config flag.
pub fn attic_command(
    c: &CacheProfile,
    token: &str,
    home: &Path,
    roots: &[String],
) -> Result<std::process::Command> {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    ensure!(
        !token.is_empty() && !roots.is_empty(),
        "missing Attic token or roots"
    );
    for root in roots {
        artifact::store_path(root)?;
    }
    let server = c
        .server
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("missing Attic endpoint"))?;
    fs::DirBuilder::new().mode(0o700).create(home)?;
    fs::create_dir(home.join("attic"))?;
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(home.join("attic/config.toml"))?;
    writeln!(
        f,
        "default-server = \"review\"\n[servers.review]\nendpoint = {}\ntoken = {}",
        serde_json::to_string(server)?,
        serde_json::to_string(token)?
    )?;
    let mut a = args(&["push", &format!("review:{}", c.cache)]);
    a.extend_from_slice(roots);
    let mut cmd = command("attic", &a, None);
    cmd.env("XDG_CONFIG_HOME", home);
    Ok(cmd)
}
pub fn manifest(root: &Path, repo: &str, rev: &str) -> Result<Value> {
    repository(repo)?;
    sha(rev)?;
    let policy: Value = read_json(&root.join("policy.json"))?;
    Ok(
        json!({"schema_version":VERSION,"repository":repo,"workflow_path":WORKFLOW,"controller_revision":rev,
        "cli":"repo-review","flake_outputs":["packages.SYSTEM.repo-review","apps.SYSTEM.repo-review"],
        "request_schema_version":VERSION,"plan_schema_version":VERSION,"result_schema_version":VERSION,
        "backends":{"flake":"implemented","external-flake":"implemented; recipe allowlist required","nixpkgs":"pinned nixpkgs-review 3.7.0 selection adapter; no fabricated live acceptance"},
        "systems":["x86_64-linux","aarch64-linux","x86_64-darwin","aarch64-darwin"],"live_acceptance_system":"x86_64-linux",
        "cache_profile_names":["attic-existing","cachix-existing"],"configured_cache_profiles":policy["caches"],
        "ready":false,"activation_gates":["Implementation PR must be reviewed and merged, not auto-merged","Workflow must be registered on default branch","Run bounded generic live acceptance and inspect evidence","Approve exact cache URL/public keys in policy.json before enabling publication","Approve each external recipe pin in policy.json","Run separate digest-bound promotion and fresh-cache retrieval acceptance"]}),
    )
}
pub fn dispatch(request: &Request, controller: &str, revision: &str) -> Result<Value> {
    request.validate()?;
    repository(controller)?;
    sha(revision)?;
    run(
        "gh",
        &args(&[
            "workflow",
            "run",
            "review-repository.yml",
            "--repo",
            controller,
            "--ref",
            revision,
            "-f",
            &format!("request={}", serde_json::to_string(request)?),
        ]),
        None,
        None,
    )?;
    Ok(
        json!({"dispatched":true,"request_id":canonical_digest(request)?,"controller":controller,"revision":revision,"status_argv":["gh","run","list","--repo",controller,"--workflow","review-repository.yml","--commit",revision,"--json","databaseId,headSha,status,conclusion"]}),
    )
}
pub fn status(repo: &str, run_id: u64, wait: u64) -> Result<Value> {
    repository(repo)?;
    ensure!(wait <= 300, "foreground wait capped at 300 seconds");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(wait);
    loop {
        let v = github::api(&format!("repos/{repo}/actions/runs/{run_id}"))?;
        if v["status"] == "completed" || std::time::Instant::now() >= deadline {
            return Ok(v);
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
pub fn retrieve_report(
    repo: &str,
    run_id: u64,
    attempt: u64,
    destination: &Path,
) -> Result<PathBuf> {
    repository(repo)?;
    ensure!(!destination.exists(), "destination must be new");
    run(
        "gh",
        &args(&[
            "run",
            "download",
            &run_id.to_string(),
            "--repo",
            repo,
            "--name",
            &format!("review-{run_id}-{attempt}"),
            "--dir",
            destination.to_str().unwrap_or(""),
        ]),
        None,
        None,
    )?;
    let r: ReviewResult = read_json(&destination.join("review-result.json"))?;
    ensure!(
        r.plan.run_id == run_id && r.plan.run_attempt == attempt && r.plan.run_repository == repo,
        "report origin mismatch"
    );
    aggregate(&r.plan, r.platforms.into_values().collect())?;
    Ok(destination.join("review-result.json"))
}
