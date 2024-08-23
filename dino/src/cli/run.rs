use std::{collections::HashMap, fs};

use clap::Parser;

use crate::{build_project, CmdExecutor, JsWorker, Request};

#[derive(Debug, Parser)]
pub struct RunOpts {}

impl CmdExecutor for RunOpts {
    async fn execute(self) -> anyhow::Result<()> {
        let (filename, _) = build_project(".")?;
        let content = fs::read_to_string(filename)?;
        let worker = JsWorker::try_new(&content)?;
        let req = Request::builder()
            .method("GET")
            .url("https://example.com")
            .headers(HashMap::new())
            .build();
        let ret = worker.run("hello", req)?;
        println!("Response: {:?}", ret);
        Ok(())
    }
}
