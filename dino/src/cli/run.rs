use clap::Parser;

use crate::CmdExecutor;

#[derive(Debug, Parser)]
pub struct RunOpts {}

impl CmdExecutor for RunOpts {
    async fn execute(self) -> anyhow::Result<()> {
        todo!()
    }
}
