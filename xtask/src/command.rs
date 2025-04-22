use std::{
    ffi::{OsStr, OsString},
    path::Path,
};

use anyhow::Context;
use command_group::{CommandGroup, GroupChild};

use crate::{fs::normalize_path, print::print_cargo_style};

pub struct Command {
    command: OsString,
    args: Vec<OsString>,
    command_full_debug: String,
}
impl Command {
    pub fn new<S1, S2, I>(command: S1, args: I) -> Self
    where
        S1: AsRef<OsStr>,
        S2: AsRef<OsStr>,
        I: IntoIterator<Item = S2>,
    {
        let command = command.as_ref().to_owned();
        let command_normalized = normalize_path(&Path::new(&command));
        let args = args
            .into_iter()
            .map(|s| s.as_ref().to_owned())
            .collect::<Vec<_>>();
        let command_full_debug = format!(
            "{} {}",
            command_normalized.display(),
            args.iter()
                .map(|arg| arg.display().to_string())
                .collect::<Vec<_>>()
                .join(" ")
        );
        Self {
            command,
            args,
            command_full_debug,
        }
    }

    pub fn new_npm_css_build(release: bool) -> Self {
        let npm_build_css_script_name = if release {
            "build-release:css"
        } else {
            "build:css"
        };
        Self::new("npm.cmd", ["run", npm_build_css_script_name])
    }

    pub fn new_cargo_build(release: bool) -> Self {
        let args = if release {
            vec!["build", "--release"]
        } else {
            vec!["build"]
        };
        Self::new(env!("CARGO"), args)
    }

    pub fn new_cargo_run(release: bool) -> Self {
        let args = if release {
            vec!["run", "--release"]
        } else {
            vec!["run"]
        };
        Self::new(env!("CARGO"), args)
    }

    pub fn print_running(&self) {
        print_cargo_style("Running", &self.command_full_debug);
    }

    pub fn builder(&self) -> std::process::Command {
        let mut command = std::process::Command::new(&self.command);
        command.args(&self.args);
        command
    }

    pub fn run(&self) -> Result<(), anyhow::Error> {
        let mut command = self.builder();
        self.print_running();
        let status = command.status().context(format!(
            "failed to run command: {}",
            &self.command_full_debug
        ))?;
        if !status.success() {
            anyhow::bail!(
                "command returned exit code {}: {}",
                status.code().unwrap_or(-1),
                &self.command_full_debug
            );
        }
        Ok(())
    }

    pub fn group_spawn(&self) -> Result<GroupChild, anyhow::Error> {
        let mut command = self.builder();
        self.print_running();
        Ok(command
            .group_spawn()
            .with_context(|| format!("failed to spawn command: {}", &self.command_full_debug))?)
    }
}
