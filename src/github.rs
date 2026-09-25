use crate::{
    canonical_digest,
    contract::*,
    process::{args, run},
    read_json,
};
use anyhow::{Result, bail, ensure};
use serde_json::Value;
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub fn api(endpoint: &str) -> Result<Value> {
    Ok(serde_json::from_str(&run(
        "gh",
        &args(&["api", "--hostname", "github.com", endpoint]),
        None,
        None,
    )?)?)
}
fn text(v: &Value, key: &str) -> Result<String> {
    Ok(v.pointer(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing GitHub field {key}"))?
        .into())
}
pub fn identity(repo: &str, revision: &str) -> Result<Identity> {
    repository(repo)?;
    reference(revision)?;
    let r = api(&format!("repos/{repo}"))?;
    let canonical = text(&r, "/full_name")?;
    repository(&canonical)?;
    let c = api(&format!("repos/{canonical}/commits/{revision}"))?;
    let i = Identity {
        id: r["id"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("missing repository id"))?,
        repository: canonical,
        commit: text(&c, "/sha")?,
        tree: text(&c, "/commit/tree/sha")?,
    };
    sha(&i.commit)?;
    sha(&i.tree)?;
    Ok(i)
}
pub fn resolve_pr(
    r: &Request,
    p: &Value,
    merge_commit: Option<&Value>,
) -> Result<(String, PullRequest)> {
    let head = text(p, "/head/sha")?;
    let base = text(p, "/base/sha")?;
    sha(&head)?;
    sha(&base)?;
    preconditions(r, &head, &base)?;
    let number = r.pr.ok_or_else(|| anyhow::anyhow!("PR required"))?;
    ensure!(p["number"].as_u64() == Some(number), "wrong PR response");
    let mut pr = PullRequest {
        number,
        url: format!("https://github.com/{}/pull/{number}", r.repository),
        head_repository: text(p, "/head/repo/full_name")?,
        head: head.clone(),
        base: base.clone(),
        merge: None,
        parents: vec![],
    };
    repository(&pr.head_repository)?;
    if r.mode == Mode::Head {
        return Ok((head, pr));
    }
    ensure!(
        p["mergeable"] == true && p["merged"] == false,
        "merge mode requires an open mergeable PR"
    );
    let merge = text(p, "/merge_commit_sha")?;
    sha(&merge)?;
    let c = merge_commit.ok_or_else(|| anyhow::anyhow!("missing exact merge commit metadata"))?;
    ensure!(c["sha"] == merge, "merge SHA mismatch");
    let parents: Vec<String> = c["parents"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("missing parents"))?
        .iter()
        .map(|v| text(v, "/sha"))
        .collect::<Result<_>>()?;
    ensure!(parents == [base, head], "wrong merge parents");
    pr.merge = Some(merge.clone());
    pr.parents = parents;
    Ok((merge, pr))
}
pub fn plan(
    mut request: Request,
    controller_repo: &str,
    controller_sha: &str,
    root: &Path,
) -> Result<Plan> {
    request.validate()?;
    sha(controller_sha)?;
    let controller = identity(controller_repo, controller_sha)?;
    ensure!(
        run("git", &args(&["rev-parse", "HEAD"]), Some(root), None)?.trim() == controller.commit,
        "controller checkout differs from workflow revision"
    );
    let canonical = identity(
        &request.repository,
        request.revision.as_deref().unwrap_or("HEAD"),
    )?;
    request.repository = canonical.repository;
    let (target, pr) = if let Some(number) = request.pr {
        let deadline = Instant::now() + Duration::from_secs(30);
        let endpoint = format!("repos/{}/pulls/{number}", request.repository);
        let initial = api(&endpoint)?;
        let head = text(&initial, "/head/sha")?;
        let base = text(&initial, "/base/sha")?;
        let mut p = initial;
        while request.mode == Mode::Merge && p["mergeable"].is_null() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_secs(2));
            p = api(&endpoint)?;
            ensure!(
                p["head"]["sha"] == head && p["base"]["sha"] == base,
                "PR moved during resolution"
            );
        }
        let merge = if request.mode == Mode::Merge && p["mergeable"] == true {
            let s = text(&p, "/merge_commit_sha")?;
            sha(&s)?;
            Some(api(&format!("repos/{}/commits/{s}", request.repository))?)
        } else {
            None
        };
        let (commit, pr) = resolve_pr(&request, &p, merge.as_ref())?;
        (identity(&request.repository, &commit)?, Some(pr))
    } else {
        (
            identity(
                &request.repository,
                request.revision.as_deref().unwrap_or("HEAD"),
            )?,
            None,
        )
    };
    if let Some(expected) = &request.expected_head {
        ensure!(
            pr.as_ref().map(|p| &p.head).unwrap_or(&target.commit) == expected,
            "head precondition failed"
        );
    }
    let recipe = if let Some(r) = &request.recipe {
        let policy: Value = read_json(&root.join("policy.json"))?;
        let own_fixture = r.repository == controller.repository
            && r.commit == controller.commit
            && r.directory == "fixtures/external";
        let trusted = policy["recipes"].as_array().is_some_and(|entries| {
            entries.iter().any(|e| {
                e["repository"] == r.repository
                    && e["commit"] == r.commit
                    && e["directory"] == r.directory
                    && e["source_input"] == r.source_input
            })
        });
        ensure!(
            own_fixture || trusted,
            "recipe pin is not in controller policy.json"
        );
        Some(identity(&r.repository, &r.commit)?)
    } else {
        None
    };
    let tool_lock: Value = read_json(&root.join("flake.lock"))?;
    let mut p = Plan {
        schema_version: VERSION,
        request_id: canonical_digest(&request)?,
        request,
        target,
        pr,
        recipe,
        controller: controller.clone(),
        workflow: WORKFLOW.into(),
        tool_lock_digest: canonical_digest(&tool_lock)?,
        tool_lock,
        run_repository: std::env::var("GITHUB_REPOSITORY").unwrap_or(controller.repository),
        run_id: std::env::var("GITHUB_RUN_ID")
            .unwrap_or("0".into())
            .parse()?,
        run_attempt: std::env::var("GITHUB_RUN_ATTEMPT")
            .unwrap_or("0".into())
            .parse()?,
        digest: String::new(),
    };
    p.seal()?;
    p.validate()?;
    Ok(p)
}
pub fn checkout(i: &Identity, destination: &Path) -> Result<()> {
    ensure!(!destination.exists(), "checkout destination already exists");
    std::fs::create_dir_all(destination)?;
    run("git", &args(&["init", "--quiet"]), Some(destination), None)?;
    run(
        "git",
        &args(&[
            "-c",
            "protocol.file.allow=never",
            "fetch",
            "--depth=1",
            "--no-tags",
            &format!("https://github.com/{}.git", i.repository),
            &i.commit,
        ]),
        Some(destination),
        None,
    )?;
    run(
        "git",
        &args(&[
            "-c",
            "core.hooksPath=/dev/null",
            "checkout",
            "--detach",
            "FETCH_HEAD",
        ]),
        Some(destination),
        None,
    )?;
    ensure!(
        run(
            "git",
            &args(&["rev-parse", "HEAD"]),
            Some(destination),
            None
        )?
        .trim()
            == i.commit,
        "checkout SHA mismatch"
    );
    ensure!(
        run(
            "git",
            &args(&["rev-parse", "HEAD^{tree}"]),
            Some(destination),
            None
        )?
        .trim()
            == i.tree,
        "checkout tree mismatch"
    );
    Ok(())
}
pub fn verify_run(repo: &str, id: u64, attempt: u64, controller: &str) -> Result<Value> {
    repository(repo)?;
    sha(controller)?;
    let r = api(&format!(
        "repos/{repo}/actions/runs/{id}/attempts/{attempt}"
    ))?;
    ensure!(
        r["repository"]["full_name"] == repo
            && r["head_sha"] == controller
            && r["path"] == WORKFLOW
            && r["event"] == "workflow_dispatch"
            && r["run_attempt"] == attempt
            && r["status"] == "completed",
        "untrusted source workflow/run/attempt/revision"
    );
    let actor = text(&r, "/triggering_actor/login")?;
    if !name(&actor) {
        bail!("invalid actor");
    }
    let permission = api(&format!("repos/{repo}/collaborators/{actor}/permission"))?;
    ensure!(
        ["admin", "maintain", "write"].contains(&permission["permission"].as_str().unwrap_or("")),
        "unauthorized source actor"
    );
    Ok(r)
}
