use anyhow::{Result, bail, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::canonical_digest;

pub const VERSION: u32 = 1;
pub const WORKFLOW: &str = ".github/workflows/review-repository.yml";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Backend {
    Nixpkgs,
    Flake,
    ExternalFlake,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    #[default]
    Head,
    Merge,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Publication {
    #[default]
    None,
    RequestApproval,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub repository: String,
    pub commit: String,
    #[serde(default = "dot")]
    pub directory: String,
    pub source_input: String,
}
fn dot() -> String {
    ".".into()
}
fn runner_default() -> String {
    "hosted-v1".into()
}
fn test_default() -> String {
    "checks-v1".into()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema_version: u32,
    pub repository: String,
    pub pr: Option<u64>,
    pub revision: Option<String>,
    #[serde(default)]
    pub mode: Mode,
    pub expected_head: Option<String>,
    pub expected_base: Option<String>,
    pub backend: Backend,
    pub recipe: Option<Recipe>,
    #[serde(default = "dot")]
    pub directory: String,
    pub systems: Vec<String>,
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub checks: Vec<String>,
    #[serde(default = "runner_default")]
    pub runner_profile: String,
    #[serde(default = "test_default")]
    pub test_profile: String,
    pub cache_profile: Option<String>,
    #[serde(default)]
    pub publication: Publication,
    #[serde(default)]
    pub post_result: bool,
}

pub fn name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s.as_bytes()[0].is_ascii_alphanumeric()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
pub fn repository(s: &str) -> Result<()> {
    let parts: Vec<_> = s.split('/').collect();
    ensure!(
        parts.len() == 2
            && name(parts[0])
            && !parts[1].is_empty()
            && parts[1].len() <= 100
            && parts[1].as_bytes()[0].is_ascii_alphanumeric()
            && parts[1]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            && !parts[1].contains(".."),
        "invalid GitHub repository coordinates"
    );
    Ok(())
}
pub fn sha(s: &str) -> Result<()> {
    ensure!(
        s.len() == 40
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "expected full lowercase commit SHA"
    );
    Ok(())
}
pub fn hash(s: &str) -> Result<()> {
    ensure!(
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "invalid SHA256 digest"
    );
    Ok(())
}
pub fn directory(s: &str) -> Result<()> {
    ensure!(
        s == "." || (!s.is_empty() && s.len() < 240 && s.split('/').all(name)),
        "unsafe directory"
    );
    Ok(())
}
pub fn reference(s: &str) -> Result<()> {
    ensure!(
        !s.is_empty()
            && s.len() <= 240
            && !s.starts_with('-')
            && !s.contains("..")
            && s.split('/').all(|p| !p.is_empty()
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))),
        "unsafe ref selector"
    );
    Ok(())
}
pub fn runner(system: &str) -> Result<(&'static str, &'static str, &'static str)> {
    match system {
        "x86_64-linux" => Ok(("ubuntu-24.04", "x86_64", "true")),
        "aarch64-linux" => Ok(("ubuntu-24.04-arm", "aarch64", "true")),
        "x86_64-darwin" => Ok(("macos-15-intel", "x86_64", "relaxed")),
        "aarch64-darwin" => Ok(("macos-15", "arm64", "relaxed")),
        _ => bail!("unsupported system"),
    }
}
fn unique(values: &[String]) -> bool {
    values.iter().collect::<BTreeSet<_>>().len() == values.len()
}
impl Request {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.schema_version == VERSION, "unsupported schema version");
        repository(&self.repository)?;
        ensure!(
            self.pr.is_some() != self.revision.is_some() && self.pr != Some(0),
            "exactly one nonzero PR or revision required"
        );
        if let Some(s) = &self.revision {
            reference(s)?;
        }
        for s in [&self.expected_head, &self.expected_base]
            .into_iter()
            .flatten()
        {
            sha(s)?;
        }
        ensure!(
            self.pr.is_some() || (self.mode == Mode::Head && self.expected_base.is_none()),
            "PR-only options on revision"
        );
        directory(&self.directory)?;
        ensure!(
            !self.systems.is_empty() && self.systems.len() <= 4 && unique(&self.systems),
            "nonempty unique systems required"
        );
        for s in &self.systems {
            runner(s)?;
        }
        ensure!(self.runner_profile == "hosted-v1", "unknown runner profile");
        ensure!(
            ["checks-v1", "checks-rebuild-v1"].contains(&self.test_profile.as_str()),
            "unknown test profile"
        );
        ensure!(
            self.packages.len() + self.checks.len() <= 64
                && unique(&self.packages)
                && unique(&self.checks),
            "too many or duplicate outputs"
        );
        for s in self.packages.iter().chain(&self.checks) {
            ensure!(name(s), "unsafe output name");
        }
        match self.backend {
            Backend::Nixpkgs => {
                ensure!(
                    self.repository.eq_ignore_ascii_case("NixOS/nixpkgs") && self.pr.is_some(),
                    "nixpkgs backend requires a NixOS/nixpkgs PR"
                );
                ensure!(
                    self.packages.is_empty()
                        && self.checks.is_empty()
                        && self.recipe.is_none()
                        && self.directory == ".",
                    "nixpkgs backend selects changed derivations, not flake outputs"
                );
            }
            Backend::Flake | Backend::ExternalFlake => {
                ensure!(
                    !self.packages.is_empty() || !self.checks.is_empty(),
                    "zero targets"
                );
                ensure!(
                    (self.backend == Backend::ExternalFlake) == self.recipe.is_some(),
                    "recipe/backend conflict"
                );
            }
        }
        if let Some(r) = &self.recipe {
            repository(&r.repository)?;
            sha(&r.commit)?;
            directory(&r.directory)?;
            ensure!(
                name(&r.source_input) && self.directory == ".",
                "invalid source input or conflicting directory"
            );
        }
        if let Some(c) = &self.cache_profile {
            ensure!(
                ["attic-existing", "cachix-existing"].contains(&c.as_str()),
                "unknown cache profile"
            );
        }
        ensure!(
            self.publication == Publication::None || self.cache_profile.is_some(),
            "publication requires named cache profile"
        );
        ensure!(
            !self.post_result || self.pr.is_some(),
            "posting requires PR"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub id: u64,
    pub repository: String,
    pub commit: String,
    pub tree: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PullRequest {
    pub number: u64,
    pub url: String,
    pub head_repository: String,
    pub head: String,
    pub base: String,
    pub merge: Option<String>,
    pub parents: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema_version: u32,
    pub request_id: String,
    pub request: Request,
    pub target: Identity,
    pub pr: Option<PullRequest>,
    pub recipe: Option<Identity>,
    pub controller: Identity,
    pub workflow: String,
    pub tool_lock: serde_json::Value,
    pub tool_lock_digest: String,
    pub run_repository: String,
    pub run_id: u64,
    pub run_attempt: u64,
    pub digest: String,
}
impl Plan {
    pub fn seal(&mut self) -> Result<()> {
        self.digest.clear();
        self.digest = canonical_digest(self)?;
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        self.request.validate()?;
        ensure!(
            self.schema_version == VERSION && self.workflow == WORKFLOW,
            "invalid plan version/workflow"
        );
        repository(&self.run_repository)?;
        for i in [&self.target, &self.controller]
            .into_iter()
            .chain(self.recipe.iter())
        {
            ensure!(i.id != 0, "missing repository identity");
            repository(&i.repository)?;
            sha(&i.commit)?;
            sha(&i.tree)?;
        }
        ensure!(
            self.target
                .repository
                .eq_ignore_ascii_case(&self.request.repository),
            "target mismatch"
        );
        ensure!(
            self.request_id == canonical_digest(&self.request)?,
            "request digest mismatch"
        );
        ensure!(
            self.tool_lock_digest == canonical_digest(&self.tool_lock)?,
            "tool lock mismatch"
        );
        if let Some(p) = &self.pr {
            repository(&p.head_repository)?;
            sha(&p.head)?;
            sha(&p.base)?;
            ensure!(
                self.request.pr == Some(p.number)
                    && p.url
                        == format!(
                            "https://github.com/{}/pull/{}",
                            self.target.repository, p.number
                        ),
                "PR identity mismatch"
            );
            preconditions(&self.request, &p.head, &p.base)?;
            match self.request.mode {
                Mode::Head => ensure!(
                    self.target.commit == p.head && p.merge.is_none() && p.parents.is_empty(),
                    "head mismatch"
                ),
                Mode::Merge => ensure!(
                    p.merge.as_ref() == Some(&self.target.commit)
                        && p.parents == [p.base.clone(), p.head.clone()],
                    "wrong merge parents"
                ),
            }
        } else {
            ensure!(self.request.pr.is_none(), "missing PR identity");
        }
        match (&self.recipe, &self.request.recipe) {
            (Some(i), Some(r)) => ensure!(
                i.repository.eq_ignore_ascii_case(&r.repository) && i.commit == r.commit,
                "recipe identity mismatch"
            ),
            (None, None) => (),
            _ => bail!("missing recipe identity"),
        }
        let mut copy = self.clone();
        copy.seal()?;
        ensure!(copy.digest == self.digest, "plan digest mismatch");
        Ok(())
    }
}
pub fn preconditions(r: &Request, head: &str, base: &str) -> Result<()> {
    ensure!(
        r.expected_head.as_deref().is_none_or(|s| s == head),
        "PR head moved"
    );
    ensure!(
        r.expected_base.as_deref().is_none_or(|s| s == base),
        "PR base moved"
    );
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Passed,
    Failed,
    Blocked,
    Unsupported,
    NotRun,
    NoChanges,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub installable: String,
    pub derivation: String,
    pub outputs: BTreeMap<String, String>,
    pub check: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EffectivePlan {
    pub metadata_digest: String,
    pub system: String,
    pub flake_reference: Option<String>,
    pub lock: Option<serde_json::Value>,
    pub lock_digest: Option<String>,
    pub source_nar_hash: Option<String>,
    pub targets: Vec<Target>,
    pub digest: String,
}
impl EffectivePlan {
    pub fn seal(&mut self) -> Result<()> {
        self.digest.clear();
        self.digest = canonical_digest(self)?;
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlatformResult {
    pub schema_version: u32,
    pub plan: Plan,
    pub effective: EffectivePlan,
    pub runner_architecture: String,
    pub nix_version: String,
    pub sandbox: String,
    pub build: Outcome,
    pub tests: Outcome,
    pub closure_export: Outcome,
    pub publication: Outcome,
    pub retrieval: Outcome,
    pub error: Option<String>,
    pub files: BTreeMap<String, String>,
    pub target_outcomes: BTreeMap<String, Outcome>,
    pub test_evidence: BTreeMap<String, String>,
    pub closure: BTreeMap<String, Nar>,
}
impl PlatformResult {
    pub fn successful(&self) -> bool {
        self.error.is_none()
            && match self.build {
                Outcome::Passed => {
                    self.closure_export == Outcome::Passed
                        && (!self.effective.targets.iter().any(|t| t.check)
                            || self.tests == Outcome::Passed)
                }
                Outcome::NoChanges => {
                    self.closure_export == Outcome::NotRun && self.tests == Outcome::NotRun
                }
                _ => false,
            }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Nar {
    pub nar_hash: String,
    pub nar_size: u64,
    pub references: Vec<String>,
}

pub fn example() -> Request {
    Request {
        schema_version: VERSION,
        repository: "OWNER/REPOSITORY".into(),
        pr: Some(1),
        revision: None,
        mode: Mode::Head,
        expected_head: None,
        expected_base: None,
        backend: Backend::Flake,
        recipe: None,
        directory: dot(),
        systems: vec!["x86_64-linux".into()],
        packages: vec!["default".into()],
        checks: vec!["smoke".into()],
        runner_profile: runner_default(),
        test_profile: test_default(),
        cache_profile: None,
        publication: Publication::None,
        post_result: false,
    }
}
