use crate::{
    backend, canonical_digest,
    contract::*,
    process::{nix_args, run},
};
use anyhow::{Result, bail, ensure};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::Path,
};

pub const MAX_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub const MAX_NAR: u64 = 8 * 1024 * 1024 * 1024;
pub fn public_url(s: &str) -> Result<()> {
    let u = url::Url::parse(s)?;
    ensure!(
        u.scheme() == "https"
            && u.host_str().is_some()
            && u.username().is_empty()
            && u.password().is_none()
            && u.query().is_none()
            && u.fragment().is_none(),
        "only credential-free HTTPS URLs permitted"
    );
    Ok(())
}
pub fn store_path(s: &str) -> Result<()> {
    let rest = s
        .strip_prefix("/nix/store/")
        .ok_or_else(|| anyhow::anyhow!("invalid store path"))?;
    ensure!(
        rest.len() > 33
            && rest.len() <= 240
            && rest.as_bytes()[32] == b'-'
            && rest.as_bytes()[..32]
                .iter()
                .all(|b| b"0123456789abcdfghijklmnpqrsvwxyz".contains(b))
            && rest[33..]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+._?=-".contains(&b)),
        "unsafe store path"
    );
    Ok(())
}
fn relative(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 240
        && s.split('/').all(|p| {
            !p.is_empty()
                && p != "."
                && p != ".."
                && !p.starts_with('-')
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        })
}
pub fn file_digest(path: &Path) -> Result<String> {
    let mut f = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buf = [0; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn inventory(root: &Path) -> Result<BTreeMap<String, String>> {
    fn walk(
        root: &Path,
        path: &Path,
        total: &mut u64,
        entries: &mut BTreeMap<String, String>,
    ) -> Result<()> {
        for item in fs::read_dir(path)? {
            let item = item?;
            let meta = fs::symlink_metadata(item.path())?;
            let rel = item
                .path()
                .strip_prefix(root)?
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("non-UTF8 artifact"))?
                .to_string();
            ensure!(
                relative(&rel) && !meta.file_type().is_symlink(),
                "unsafe artifact path"
            );
            if meta.is_dir() {
                walk(root, &item.path(), total, entries)?;
            } else {
                ensure!(meta.is_file(), "special artifact file");
                ensure!(
                    rel.starts_with("cache/")
                        || [".json", ".log", ".md", ".lock"]
                            .iter()
                            .any(|s| rel.ends_with(s)),
                    "unexpected executable hook/file in artifact"
                );
                *total = total
                    .checked_add(meta.len())
                    .ok_or_else(|| anyhow::anyhow!("artifact size overflow"))?;
                ensure!(
                    *total <= MAX_BYTES && entries.len() < 100000,
                    "artifact too large"
                );
                if rel != "review-result.json" {
                    entries.insert(rel, file_digest(&item.path())?);
                }
            }
        }
        Ok(())
    }
    let mut entries = BTreeMap::new();
    walk(root, root, &mut 0, &mut entries)?;
    Ok(entries)
}
pub fn roots(r: &PlatformResult) -> Vec<String> {
    r.effective
        .targets
        .iter()
        .flat_map(|t| t.outputs.values().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
pub fn closure_info(paths: &[String], store: Option<&str>) -> Result<BTreeMap<String, Nar>> {
    ensure!(!paths.is_empty(), "zero roots");
    for p in paths {
        store_path(p)?;
    }
    let mut a = nix_args(&["path-info", "--recursive", "--json"]);
    if let Some(s) = store {
        a.extend(["--store".into(), s.into()]);
    }
    a.extend(paths.iter().cloned());
    let v: Value = serde_json::from_str(&run("nix", &a, None, None)?)?;
    let mut map = BTreeMap::new();
    let items: Vec<(String, Value)> = match v {
        Value::Object(m) => m.into_iter().collect(),
        Value::Array(a) => a
            .into_iter()
            .map(|v| (v["path"].as_str().unwrap_or("").to_owned(), v))
            .collect(),
        _ => bail!("invalid path-info result"),
    };
    for (path, v) in items {
        store_path(&path)?;
        map.insert(
            path,
            Nar {
                nar_hash: v["narHash"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("missing NAR hash"))?
                    .into(),
                nar_size: v["narSize"]
                    .as_u64()
                    .ok_or_else(|| anyhow::anyhow!("missing NAR size"))?,
                references: serde_json::from_value(v["references"].clone())?,
            },
        );
    }
    validate_closure(paths, &map)?;
    Ok(map)
}
pub fn validate_closure(roots: &[String], map: &BTreeMap<String, Nar>) -> Result<()> {
    ensure!(
        !roots.is_empty() && !map.is_empty() && map.len() <= 100000,
        "empty/oversized closure"
    );
    let mut total = 0u64;
    for (p, n) in map {
        store_path(p)?;
        ensure!(
            n.nar_hash.starts_with("sha256:") || n.nar_hash.starts_with("sha256-"),
            "invalid NAR hash"
        );
        ensure!(n.nar_hash.len() <= 80, "invalid NAR hash size");
        total = total
            .checked_add(n.nar_size)
            .ok_or_else(|| anyhow::anyhow!("NAR size overflow"))?;
        ensure!(total <= MAX_NAR, "closure exceeds NAR limit");
        for r in &n.references {
            store_path(r)?;
            ensure!(map.contains_key(r), "missing runtime dependency");
        }
    }
    let mut seen = BTreeSet::new();
    let mut pending = roots.to_vec();
    while let Some(p) = pending.pop() {
        ensure!(map.contains_key(&p), "missing closure root");
        if seen.insert(p.clone()) {
            pending.extend(map[&p].references.clone());
        }
    }
    ensure!(
        seen.len() == map.len(),
        "unreachable paths in closure manifest"
    );
    Ok(())
}
pub fn export(result: &mut PlatformResult, out: &Path) -> Result<()> {
    let roots = roots(result);
    result.closure = closure_info(&roots, None)?;
    let cache = out.join("cache");
    fs::create_dir(&cache)?;
    let mut a = nix_args(&[
        "copy",
        "--to",
        &format!("file://{}", cache.canonicalize()?.display()),
    ]);
    a.extend(roots);
    run("nix", &a, None, Some(&out.join("export.log")))?;
    Ok(())
}
pub fn validate_result(plan: &Plan, result: &PlatformResult) -> Result<()> {
    plan.validate()?;
    result.plan.validate()?;
    ensure!(
        result.schema_version == VERSION && result.plan == *plan,
        "result does not match trusted metadata plan"
    );
    let e = &result.effective;
    ensure!(
        result.publication == Outcome::NotRun && result.retrieval == Outcome::NotRun,
        "build reports cannot attest publication or retrieval"
    );
    ensure!(
        matches!(
            result.closure_export,
            Outcome::Passed | Outcome::Failed | Outcome::NotRun
        ),
        "invalid closure-export outcome"
    );
    ensure!(
        result.closure_export != Outcome::Passed || result.error.is_none(),
        "successful export with execution error"
    );
    ensure!(
        e.metadata_digest == plan.digest && plan.request.systems.contains(&e.system),
        "wrong platform/metadata"
    );
    let mut copy = e.clone();
    copy.seal()?;
    ensure!(copy.digest == e.digest, "effective plan digest mismatch");
    let (_, arch, sandbox) = runner(&e.system)?;
    // Failure reports can legitimately stop before runner inspection.
    if matches!(result.build, Outcome::Passed | Outcome::NoChanges) {
        ensure!(
            result.runner_architecture == arch
                && result.sandbox == sandbox
                && !result.nix_version.is_empty(),
            "wrong runner platform/security facts"
        );
    }
    let nixpkgs = plan.request.backend == Backend::Nixpkgs;
    ensure!(
        nixpkgs || result.build != Outcome::NoChanges,
        "no_changes is Nixpkgs-only"
    );
    if e.targets.is_empty() {
        ensure!(
            result.tests == Outcome::NotRun
                && result.closure_export == Outcome::NotRun
                && result.target_outcomes.is_empty()
                && result.test_evidence.is_empty()
                && result.closure.is_empty(),
            "empty selection cannot attest execution or export"
        );
        ensure!(
            matches!(
                result.build,
                Outcome::Blocked | Outcome::Unsupported | Outcome::Failed
            ) || (nixpkgs && result.build == Outcome::NoChanges),
            "absent targets cannot pass"
        );
        return Ok(());
    }
    if !nixpkgs {
        ensure!(
            e.flake_reference.as_deref() == Some(&backend::flake_ref(plan)),
            "wrong recipe reference"
        );
        let lock = e
            .lock
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("missing effective lock"))?;
        backend::validate_lock(plan, lock)?;
        ensure!(
            e.lock_digest.as_deref() == Some(&canonical_digest(lock)?),
            "lock digest mismatch"
        );
        let expected: BTreeSet<_> = [
            ("packages", &plan.request.packages),
            ("checks", &plan.request.checks),
        ]
        .into_iter()
        .flat_map(|(k, names)| {
            names.iter().map(move |n| {
                (
                    format!("{}#{k}.{}.{n}", backend::flake_ref(plan), e.system),
                    k == "checks",
                )
            })
        })
        .collect();
        let actual: BTreeSet<_> = e
            .targets
            .iter()
            .map(|t| (t.installable.clone(), t.check))
            .collect();
        ensure!(
            expected == actual && e.targets.len() == expected.len(),
            "missing/unrequested outputs"
        );
    } else {
        ensure!(
            e.flake_reference.is_none() && e.lock.is_none(),
            "legacy backend must not invent a flake lock"
        );
        for t in &e.targets {
            ensure!(
                t.installable
                    .starts_with(&format!("nixpkgs:{}/{}/", plan.target.commit, e.system)),
                "wrong Nixpkgs installable"
            );
        }
    }
    for t in &e.targets {
        store_path(&t.derivation)?;
        ensure!(
            t.derivation.ends_with(".drv") && !t.outputs.is_empty(),
            "invalid derivation"
        );
        for (n, p) in &t.outputs {
            ensure!(name(n), "unsafe output name");
            store_path(p)?;
        }
        if result.build == Outcome::Passed {
            ensure!(
                result.target_outcomes.get(&t.installable) == Some(&Outcome::Passed),
                "skipped target"
            );
            ensure!(
                !t.check || result.tests == Outcome::Passed,
                "skipped required check"
            );
        }
        if t.check && result.tests == Outcome::Passed {
            let evidence = result.test_evidence.get(&t.installable).map(String::as_str);
            ensure!(evidence.is_some(), "missing test evidence");
            if plan.request.test_profile == "checks-rebuild-v1" {
                ensure!(
                    evidence == Some("explicit-check-rebuild"),
                    "required execution skipped"
                );
            }
        }
    }
    if result.build == Outcome::Passed {
        ensure!(
            plan.request.checks.is_empty() || result.tests == Outcome::Passed,
            "required checks did not pass"
        );
        ensure!(
            result.closure_export != Outcome::NotRun,
            "passing build must attempt closure export"
        );
    }
    if result.closure_export == Outcome::Passed {
        ensure!(
            result.build == Outcome::Passed,
            "export requires passing build"
        );
        validate_closure(&roots(result), &result.closure)?;
    }
    Ok(())
}
pub fn validate_bundle(plan: &Plan, r: &PlatformResult, path: &Path) -> Result<String> {
    validate_result(plan, r)?;
    ensure!(
        inventory(path)? == r.files,
        "artifact inventory/digests mismatch"
    );
    if matches!(r.build, Outcome::Passed | Outcome::NoChanges) {
        ensure!(
            r.files.contains_key("effective-plan.json"),
            "missing frozen plan"
        );
        let ep: EffectivePlan = crate::read_json(&path.join("effective-plan.json"))?;
        ensure!(ep == r.effective, "effective plan file mismatch");
        if plan.request.backend == Backend::Nixpkgs {
            let selection: Value = crate::read_json(&path.join("nixpkgs-selection.json"))?;
            ensure!(
                selection["tested_commit"] == plan.target.commit
                    && selection["base_commit"]
                        == plan
                            .pr
                            .as_ref()
                            .ok_or_else(|| anyhow::anyhow!("missing PR"))?
                            .base
                    && selection["system"] == r.effective.system
                    && selection["backend_version"] == "3.7.0",
                "wrong Nixpkgs selection identity"
            );
            let selected = selection["derivations"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("missing selection report"))?;
            ensure!(
                selected.len() == r.effective.targets.len(),
                "missing selected Nixpkgs target"
            );
            for (s, t) in selected.iter().zip(&r.effective.targets) {
                ensure!(
                    s["derivation"] == t.derivation
                        && s["check"] == t.check
                        && t.installable
                            == format!(
                                "nixpkgs:{}/{}/{}",
                                plan.target.commit,
                                r.effective.system,
                                s["attribute"].as_str().unwrap_or("")
                            ),
                    "selected Nixpkgs derivation mismatch"
                );
            }
            if r.build == Outcome::NoChanges {
                ensure!(
                    selection["changed_attributes"] == serde_json::json!([]) && selected.is_empty(),
                    "no_changes requires successful empty selection"
                );
            }
        } else {
            ensure!(
                r.files.contains_key("effective.lock"),
                "missing frozen lock"
            );
            let lock: Value = crate::read_json(&path.join("effective.lock"))?;
            ensure!(
                Some(lock) == r.effective.lock,
                "effective lock file mismatch"
            );
        }
        if r.closure_export == Outcome::Passed {
            validate_cache_files(r, path)?;
        }
    }
    canonical_digest(r)
}
fn validate_cache_files(r: &PlatformResult, path: &Path) -> Result<()> {
    let cache = path.join("cache");
    for p in r.closure.keys() {
        let base = p.strip_prefix("/nix/store/").unwrap_or("");
        let info_path = cache.join(format!("{}.narinfo", &base[..32]));
        let meta = fs::metadata(&info_path)?;
        ensure!(meta.len() < 65536, "oversized narinfo");
        let info = fs::read_to_string(info_path)?;
        let fields: BTreeMap<_, _> = info.lines().filter_map(|l| l.split_once(": ")).collect();
        ensure!(
            fields.get("StorePath") == Some(&p.as_str()),
            "narinfo store path mismatch"
        );
        let url = fields
            .get("URL")
            .ok_or_else(|| anyhow::anyhow!("missing NAR URL"))?;
        ensure!(
            relative(url) && url.starts_with("nar/") && url.split('/').count() == 2,
            "unsafe NAR URL"
        );
        ensure!(
            r.files.contains_key(&format!("cache/{url}")),
            "missing NAR object"
        );
        let size: u64 = fields
            .get("NarSize")
            .ok_or_else(|| anyhow::anyhow!("missing NAR size"))?
            .parse()?;
        ensure!(size == r.closure[p].nar_size, "NAR size mismatch");
        let refs: BTreeSet<_> = fields
            .get("References")
            .unwrap_or(&"")
            .split_whitespace()
            .map(|s| format!("/nix/store/{s}"))
            .collect();
        ensure!(
            refs == r.closure[p].references.iter().cloned().collect(),
            "narinfo reference mismatch"
        );
    }
    for file in r.files.keys().filter(|p| p.starts_with("cache/")) {
        ensure!(
            file == "cache/nix-cache-info"
                || file.starts_with("cache/nar/")
                || (file.ends_with(".narinfo") && file.split('/').count() == 2),
            "unexpected cache file"
        );
    }
    Ok(())
}

/// Fresh, isolated store; exact paths only. No eval, realise, build, or fallback.
pub fn retrieve(
    r: &PlatformResult,
    cache_url: &str,
    keys: &[String],
    destination: &Path,
    local_transfer: bool,
) -> Result<()> {
    let roots = roots(r);
    validate_closure(&roots, &r.closure)?;
    ensure!(
        !destination.exists(),
        "retrieval store must not exist (preinstalled outputs invalidate proof)"
    );
    if !local_transfer {
        public_url(cache_url)?;
        ensure!(!keys.is_empty(), "approved public keys required");
    }
    let store = format!("local?root={}", destination.display());
    let mut a = nix_args(&["copy", "--from", cache_url, "--to", &store]);
    if local_transfer {
        a.push("--no-check-sigs".into());
    } else {
        a.extend([
            "--option".into(),
            "extra-trusted-public-keys".into(),
            keys.join(" "),
        ]);
    }
    a.extend(roots.clone());
    run("nix", &a, None, None)?;
    ensure!(
        closure_info(&roots, Some(&store))? == r.closure,
        "retrieved closure identity mismatch"
    );
    Ok(())
}
