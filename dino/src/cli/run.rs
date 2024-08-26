use std::fs;

use clap::Parser;
use dino_server::{JsWorker, Req};

use crate::{build_project, CmdExecutor};

#[derive(Debug, Parser)]
pub struct RunOpts {}

impl CmdExecutor for RunOpts {
    async fn execute(self) -> anyhow::Result<()> {
        let (filename, _) = build_project(".")?;
        let content = fs::read_to_string(filename)?;
        let worker = JsWorker::try_new(&content)?;
        let req = Req::builder()
            .method("GET")
            .url("https://example.com")
            .build();
        let ret = worker.run("hello", req)?;
        println!("Response: {:?}", ret);
        Ok(())
    }
}
