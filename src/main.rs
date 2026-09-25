use anyhow::{Result, ensure};
use clap::{Parser, Subcommand};
use repo_review::{artifact, backend, contract::*, github, read_json, service, write_json};
use serde_json::{Value, json};
use std::{io::Read, path::PathBuf};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}
#[derive(Subcommand)]
enum Cmd {
    Schema {
        #[arg(default_value = "request")]
        kind: String,
    },
    Example {
        #[arg(long)]
        external_template: bool,
    },
    Validate {
        #[arg(default_value = "-")]
        request: String,
    },
    Plan {
        #[arg(default_value = "-")]
        request: String,
        #[arg(long)]
        controller: String,
        #[arg(long)]
        revision: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Dispatch {
        #[arg(default_value = "-")]
        request: String,
        #[arg(long)]
        controller: String,
        #[arg(long)]
        revision: String,
    },
    Status {
        #[arg(long)]
        controller: String,
        #[arg(long)]
        run: u64,
        #[arg(long, default_value_t = 0)]
        wait_seconds: u64,
    },
    Report {
        #[arg(long)]
        controller: String,
        #[arg(long)]
        run: u64,
        #[arg(long)]
        attempt: u64,
        #[arg(long)]
        output: PathBuf,
    },
    ValidateReport {
        #[arg(long)]
        plan: PathBuf,
        #[arg(long)]
        bundle: PathBuf,
    },
    Build {
        #[arg(long)]
        plan: PathBuf,
        #[arg(long)]
        system: String,
        #[arg(long)]
        output: PathBuf,
    },
    Collect {
        #[arg(long)]
        plan: PathBuf,
        #[arg(long)]
        expected_digest: String,
        #[arg(long)]
        inputs: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Fetch {
        #[arg(long)]
        result: PathBuf,
        #[arg(long)]
        policy: PathBuf,
        #[arg(long)]
        profile: String,
        #[arg(long)]
        destination: PathBuf,
    },
    VerifyLocal {
        #[arg(long)]
        plan: PathBuf,
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        destination: PathBuf,
    },
    Publish {
        #[arg(long)]
        plan: PathBuf,
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        policy: PathBuf,
        #[arg(long)]
        approved_plan: String,
        #[arg(long)]
        approved_bundle: String,
    },
    VerifyRun {
        #[arg(long)]
        controller: String,
        #[arg(long)]
        run: u64,
        #[arg(long)]
        attempt: u64,
        #[arg(long)]
        revision: String,
    },
    Post {
        #[arg(long)]
        result: PathBuf,
        #[arg(long)]
        expected_plan_digest: String,
        #[arg(long)]
        output: PathBuf,
    },
    Manifest {
        #[arg(long)]
        controller: String,
        #[arg(long)]
        revision: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    Legacy {
        #[arg(long)]
        inputs: String,
    },
}
fn request(path: &str) -> Result<Request> {
    let r: Request = if path == "-" {
        let mut b = String::new();
        std::io::stdin().take(65537).read_to_string(&mut b)?;
        ensure!(b.len() <= 65536, "request too large");
        serde_json::from_str(&b)?
    } else {
        read_json(&PathBuf::from(path))?
    };
    r.validate()?;
    Ok(r)
}
fn execute(cli: Cli) -> Result<(Value, i32)> {
    let value = match cli.command {
        Cmd::Schema { kind } => match kind.as_str() {
            "request" => serde_json::to_value(schemars::schema_for!(Request))?,
            "plan" => serde_json::to_value(schemars::schema_for!(Plan))?,
            "platform-result" => serde_json::to_value(schemars::schema_for!(PlatformResult))?,
            "result" => serde_json::to_value(schemars::schema_for!(service::ReviewResult))?,
            _ => anyhow::bail!("unknown schema"),
        },
        Cmd::Example { external_template } => {
            let mut v = serde_json::to_value(example())?;
            if external_template {
                v["repository"] = json!("SOURCE_OWNER/SOURCE_REPOSITORY");
                v["pr"] = Value::Null;
                v["revision"] = json!("REPLACE_EXACT_SOURCE_COMMIT");
                v["expected_head"] = json!("REPLACE_EXACT_SOURCE_COMMIT");
                v["backend"] = json!("external-flake");
                v["recipe"] = json!({"repository":"REPLACE_OWNER/REPLACE_RECIPE_REPOSITORY","commit":"REPLACE_EXACT_RECIPE_COMMIT","directory":".","source_input":"source"});
                v["packages"] = json!(["desktop", "cli", "mcp"]);
                v["checks"] = json!(["smoke"]);
                v["test_profile"] = json!("checks-rebuild-v1");
            }
            v
        }
        Cmd::Validate { request: p } => serde_json::to_value(request(&p)?)?,
        Cmd::Plan {
            request: p,
            controller,
            revision,
            root,
            output,
        } => {
            let p = github::plan(request(&p)?, &controller, &revision, &root)?;
            write_json(&output, &p)?;
            json!({"digest":p.digest,"request_id":p.request_id,"matrix":{"include":p.request.systems.iter().map(|s| json!({"system":s,"runner":runner(s).unwrap().0})).collect::<Vec<_>>()}})
        }
        Cmd::Dispatch {
            request: p,
            controller,
            revision,
        } => service::dispatch(&request(&p)?, &controller, &revision)?,
        Cmd::Status {
            controller,
            run,
            wait_seconds,
        } => service::status(&controller, run, wait_seconds)?,
        Cmd::Report {
            controller,
            run,
            attempt,
            output,
        } => json!({"path":service::retrieve_report(&controller, run, attempt, &output)?}),
        Cmd::ValidateReport { plan, bundle } => {
            let p: Plan = read_json(&plan)?;
            let r: PlatformResult = read_json(&bundle.join("review-result.json"))?;
            json!({"bundle_digest":artifact::validate_bundle(&p,&r,&bundle)?,"effective_plan_digest":r.effective.digest})
        }
        Cmd::Build {
            plan,
            system,
            output,
        } => {
            let r = backend::build(&read_json(&plan)?, &system, &output)?;
            let code = if matches!(r.build, Outcome::Passed | Outcome::NoChanges) {
                0
            } else if r.build == Outcome::Unsupported {
                4
            } else {
                3
            };
            return Ok((serde_json::to_value(r)?, code));
        }
        Cmd::Collect {
            plan,
            expected_digest,
            inputs,
            output,
        } => {
            let p: Plan = read_json(&plan)?;
            ensure!(
                p.digest == expected_digest,
                "trusted resolver digest mismatch"
            );
            let r = service::collect(&p, &inputs, &output)?;
            let code = if r.build == Outcome::Passed { 0 } else { 3 };
            return Ok((serde_json::to_value(r)?, code));
        }
        Cmd::Fetch {
            result,
            policy,
            profile,
            destination,
        } => {
            let r: PlatformResult = read_json(&result)?;
            artifact::validate_result(&r.plan, &r)?;
            let c = service::cache_profile(&policy, &profile)?;
            artifact::retrieve(&r, &c.url, &c.public_keys, &destination, false)?;
            json!({"retrieval":"passed","cache_url":c.url,"effective_plan_digest":r.effective.digest})
        }
        Cmd::VerifyLocal {
            plan,
            bundle,
            destination,
        } => {
            let p: Plan = read_json(&plan)?;
            let r: PlatformResult = read_json(&bundle.join("review-result.json"))?;
            artifact::validate_bundle(&p, &r, &bundle)?;
            artifact::retrieve(
                &r,
                &format!("file://{}", bundle.join("cache").canonicalize()?.display()),
                &[],
                &destination,
                true,
            )?;
            json!({"local_transfer":"passed","remote_cache_retrieval":"not_run"})
        }
        Cmd::Publish {
            plan,
            bundle,
            policy,
            approved_plan,
            approved_bundle,
        } => {
            let p: Plan = read_json(&plan)?;
            let r: PlatformResult = read_json(&bundle.join("review-result.json"))?;
            service::publish(&p, &r, &bundle, &policy, &approved_plan, &approved_bundle)?
        }
        Cmd::VerifyRun {
            controller,
            run,
            attempt,
            revision,
        } => github::verify_run(&controller, run, attempt, &revision)?,
        Cmd::Post {
            result,
            expected_plan_digest,
            output,
        } => {
            let r: service::ReviewResult = read_json(&result)?;
            ensure!(
                r.plan.digest == expected_plan_digest,
                "report does not match trusted resolver digest"
            );
            let verified = service::aggregate(&r.plan, r.platforms.values().cloned().collect())?;
            ensure!(
                verified.build == r.build
                    && verified.tests == r.tests
                    && verified.publication == r.publication
                    && verified.retrieval == r.retrieval,
                "aggregate status mismatch"
            );
            service::post(&r, &output)?
        }
        Cmd::Manifest {
            controller,
            revision,
            root,
        } => service::manifest(&root, &controller, &revision)?,
        Cmd::Legacy { inputs } => {
            let v: Value = serde_json::from_str(&inputs)?;
            ensure!(
                v["extra-args"].as_str().unwrap_or("").is_empty()
                    && v["upterm"] != true
                    && v["upterm"] != "true"
                    && v["on-success"].as_str().unwrap_or("nothing") == "nothing"
                    && v["push-to-cache"] != true
                    && v["push-to-cache"] != "true",
                "legacy shell/remote session/approval/automatic publication options are rejected"
            );
            let mut r = example();
            r.repository = "NixOS/nixpkgs".into();
            r.backend = Backend::Nixpkgs;
            r.mode = Mode::Merge;
            r.pr = Some(v["pr"].as_str().unwrap_or("").parse()?);
            r.packages.clear();
            r.checks.clear();
            r.systems.clear();
            for s in [
                "x86_64-linux",
                "aarch64-linux",
                "x86_64-darwin",
                "aarch64-darwin",
            ] {
                if v[s] == true || v[s] == "true" || v[s] == "yes_sandbox_relaxed" {
                    r.systems.push(s.into());
                } else {
                    ensure!(
                        v[s].is_null() || v[s] == false || v[s] == "false" || v[s] == "no",
                        "unsupported legacy sandbox policy"
                    );
                }
            }
            r.post_result = v["post-result"] == true || v["post-result"] == "true";
            r.validate()?;
            serde_json::to_value(r)?
        }
    };
    Ok((value, 0))
}
fn main() {
    match execute(Cli::parse()) {
        Ok((v, code)) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&v).expect("serializable JSON")
            );
            std::process::exit(code);
        }
        Err(e) => {
            eprintln!(
                "{}",
                json!({"schema_version":VERSION,"outcome":"blocked","error":e.to_string()})
            );
            std::process::exit(2);
        }
    }
}
