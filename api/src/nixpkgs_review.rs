use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tera::Tera;

use crate::github::oidc::Claims;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub pr: String,
    pub extra_args: String,
    pub head: String,
    pub merge: String,
    pub base_ref: String,
    pub systems: Vec<SystemReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemReport {
    pub system: String,
    pub sandbox: String,
    pub fetch_cmd: Option<String>,

    pub broken: Vec<Attr>,
    pub non_existent: Vec<Attr>,
    pub blacklisted: Vec<Attr>,
    pub failed: Vec<Attr>,
    pub still_failing: Vec<Attr>,
    pub tests: Vec<Attr>,
    pub built: Vec<Attr>,
    pub unsupported: Vec<Attr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attr {
    pub name: String,
    pub aliases: Vec<String>,
}

pub struct ReportMarkdownRenderer(Tera);

impl ReportMarkdownRenderer {
    pub fn new() -> Self {
        let mut tera = Tera::default();
        tera.add_raw_template("report", include_str!("../templates/report.md"))
            .unwrap();
        tera.register_function(
            "system_has_rebuilds",
            |args: &HashMap<String, tera::Value>| {
                let report = args.get("report").ok_or_else(|| {
                    tera::Error::msg("system_has_rebuilds: argument 'report' is missing")
                })?;
                let report = tera::from_value::<SystemReport>(report.clone())?;
                Ok(report.has_rebuilds().into())
            },
        );
        Self(tera)
    }

    pub fn render(&self, report: &Report, reporter: &Claims) -> String {
        #[derive(Serialize)]
        struct Context<'a> {
            #[serde(flatten)]
            report: &'a Report,
            #[serde(flatten)]
            reporter: &'a Claims,
            success: bool,
            logs_url: &'a str,
        }

        let ctx = tera::Context::from_serialize(Context {
            report,
            reporter,
            success: report.is_success(),
            logs_url: &reporter.logs_url(),
        })
        .unwrap();

        self.0.render("report", &ctx).unwrap()
    }
}

impl Report {
    pub fn is_success(&self) -> bool {
        self.systems.iter().all(|x| x.is_success())
    }
}

impl SystemReport {
    fn has_rebuilds(&self) -> bool {
        let SystemReport {
            system: _,
            sandbox: _,
            fetch_cmd: _,

            broken,
            non_existent,
            blacklisted,
            failed,
            still_failing,
            tests,
            built,
            unsupported,
        } = self;

        !broken.is_empty()
            || !non_existent.is_empty()
            || !blacklisted.is_empty()
            || !failed.is_empty()
            || !still_failing.is_empty()
            || !tests.is_empty()
            || !built.is_empty()
            || !unsupported.is_empty()
    }

    fn is_success(&self) -> bool {
        self.failed.is_empty() && self.still_failing.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render() {
        insta::assert_snapshot!(ReportMarkdownRenderer::new().render(&report(), &reporter()));
    }

    #[test]
    fn render_no_extra_args() {
        let report = Report {
            extra_args: String::new(),
            ..report()
        };

        insta::assert_snapshot!(ReportMarkdownRenderer::new().render(&report, &reporter()));
    }

    #[test]
    fn render_no_cache() {
        let report = Report {
            systems: report()
                .systems
                .into_iter()
                .map(|x| SystemReport {
                    fetch_cmd: x.fetch_cmd.map(|_| String::new()),
                    ..x
                })
                .collect(),
            ..report()
        };

        insta::assert_snapshot!(ReportMarkdownRenderer::new().render(&report, &reporter()));
    }

    #[test]
    fn render_success() {
        let report = Report {
            systems: report()
                .systems
                .into_iter()
                .map(|x| SystemReport {
                    failed: Vec::new(),
                    still_failing: Vec::new(),
                    ..x
                })
                .collect(),
            ..report()
        };

        insta::assert_snapshot!(ReportMarkdownRenderer::new().render(&report, &reporter()));
    }

    fn report() -> Report {
        Report {
            pr: "1337".into(),
            extra_args: "-a extra-package".into(),
            head: "842d0b3850da7fb970fd81c60b7527ff8e3a3c63".into(),
            merge: "d8b086693fa2d763b675ecf2373f7a3b8ca9755d".into(),
            base_ref: "master".into(),
            systems: vec![
                SystemReport {
                    system: "x86_64-linux".into(),
                    sandbox: "true".into(),
                    fetch_cmd: Some(
                        "nix-store -r \\\n  /nix/store/zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz-foo-0.0.0"
                            .into(),
                    ),
                    broken: vec![pkg("broken_pkg")],
                    non_existent: vec![pkg("non_existent_pkg")],
                    blacklisted: vec![pkg("blacklisted_pkg")],
                    failed: vec![pkg("failed_pkg")],
                    still_failing: vec![pkg("still_failing_pkg")],
                    tests: vec![pkg("tests_pkg")],
                    built: vec![Attr {
                        name: "built_pkg".into(),
                        aliases: vec!["foo".into(), "bar".into()],
                    }],
                    unsupported: vec![pkg("unsupported_pkg")],
                },
                SystemReport {
                    system: "aarch64-linux".into(),
                    sandbox: "true".into(),
                    fetch_cmd: None,
                    broken: vec![],
                    non_existent: vec![],
                    blacklisted: vec![],
                    failed: vec![],
                    still_failing: vec![],
                    tests: vec![],
                    built: vec![],
                    unsupported: vec![],
                },
                SystemReport {
                    system: "aarch64-darwin".into(),
                    sandbox: "relaxed".into(),
                    fetch_cmd: Some(
                        "nix-store -r \\\n  /nix/store/yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy-asdf-0.0.0"
                            .into(),
                    ),
                    broken: vec![],
                    non_existent: vec![],
                    blacklisted: vec![],
                    failed: vec![],
                    still_failing: vec![],
                    tests: vec![],
                    built: vec![pkg("asdf")],
                    unsupported: vec![],
                },
            ],
        }
    }

    fn reporter() -> Claims {
        Claims {
            actor: "some-user".into(),
            actor_id: "123456".into(),
            repository: "some-user/nixpkgs-review-gha".into(),
            run_attempt: "1".into(),
            run_id: "42".into(),
            workflow: "review".into(),
            workflow_sha: "f0bf93978802df847890d6f70aa57464cfab48f3".into(),
        }
    }

    fn pkg(name: impl Into<String>) -> Attr {
        Attr {
            name: name.into(),
            aliases: Vec::new(),
        }
    }
}
