use std::env;

use clap::Parser;

use crate::{build_project, CmdExecutor};

#[derive(Debug, Parser)]
pub struct BuildOpts {}

impl CmdExecutor for BuildOpts {
    async fn execute(self) -> anyhow::Result<()> {
        let cur_dir = env::current_dir()?.display().to_string();
        let (filename, cached) = build_project(&cur_dir)?;
        if cached {
            eprintln!("Build success: {} (cached)", filename);
        } else {
            eprintln!("Build success: {}", filename);
        }
        Ok(())
    }
}
