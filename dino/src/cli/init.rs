use std::{fs, path::Path};

use clap::Parser;
use dialoguer::Input;

use crate::CmdExecutor;

#[derive(Debug, Parser)]
pub struct InitOpts {}

impl CmdExecutor for InitOpts {
    async fn execute(self) -> anyhow::Result<()> {
        let name: String = Input::new().with_prompt("Project name").interact_text()?;

        // if current dir is empty, init project in current dir.
        // otherwise, create a new dir with the name and init project in it.
        let cur = Path::new(".");
        if fs::read_dir(cur)?.next().is_none() {
            init_project(&name, cur)?;
        } else {
            let path = cur.join(&name);
            init_project(&name, &path)?;
        }
        Ok(())
    }
}

fn init_project(name: &str, path: &Path) -> anyhow::Result<()> {
    todo!()
}
