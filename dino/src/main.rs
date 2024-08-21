use clap::Parser;
use dino::{CmdExecutor, Opts};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();
    opts.cmd.execute().await?;
    Ok(())
}
