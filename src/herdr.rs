use serde_json::Value;
use std::process::Command;

use crate::ssh::{remote_herdr_command, ssh_output};

pub trait HerdrHost {
    fn json(&self, args: &[String]) -> Result<Value, String>;
    fn run(&self, args: &[String]) -> Result<(), String>;
}

pub struct LocalHerdr;

pub struct SshHerdr {
    target: String,
}

impl SshHerdr {
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
        }
    }
}

impl HerdrHost for LocalHerdr {
    fn json(&self, args: &[String]) -> Result<Value, String> {
        let out = command_output("herdr", args)?;
        serde_json::from_slice(&out).map_err(|err| format!("invalid herdr json: {err}"))
    }

    fn run(&self, args: &[String]) -> Result<(), String> {
        command_output("herdr", args).map(|_| ())
    }
}

impl HerdrHost for SshHerdr {
    fn json(&self, args: &[String]) -> Result<Value, String> {
        let out = ssh_output(&self.target, &remote_herdr_command(args))?;
        serde_json::from_slice(&out).map_err(|err| format!("invalid remote herdr json: {err}"))
    }

    fn run(&self, args: &[String]) -> Result<(), String> {
        ssh_output(&self.target, &remote_herdr_command(args)).map(|_| ())
    }
}

pub fn herdr_json<I, S>(args: I) -> Result<Value, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    LocalHerdr.json(&to_args(args))
}

pub fn run_herdr<I, S>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    LocalHerdr.run(&to_args(args))
}

pub fn to_args<I, S>(args: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter().map(|s| s.as_ref().to_string()).collect()
}

fn command_output(program: &str, args: &[String]) -> Result<Vec<u8>, String> {
    let out = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run {program}: {err}"))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}
