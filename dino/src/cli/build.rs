use clap::Parser;

use crate::CmdExecutor;

#[derive(Debug, Parser)]
pub struct BuildOpts {}

impl CmdExecutor for BuildOpts {
    async fn execute(self) -> anyhow::Result<()> {
        todo!()
    }
}
