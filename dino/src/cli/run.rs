use std::{fs, time::Duration};

use clap::Parser;
use dino_server::{
    start_server, ProjectConfig, SwappableAppRouter, SwappableWorkerPool, TenentRouter,
    TenentWorkerPool,
};
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult};
use tokio::sync::mpsc::channel;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tracing::{info, level_filters::LevelFilter, warn};
use tracing_subscriber::{fmt::Layer, layer::SubscriberExt, util::SubscriberInitExt, Layer as _};

use crate::{build_project, CmdExecutor};

const MONITOR_FS_INTERVAL: Duration = Duration::from_secs(2);
const WOERK_POOL_SIZE: usize = 10;

#[derive(Debug, Parser)]
pub struct RunOpts {
    #[arg(short, long, default_value_t = 3000)]
    pub port: u16,
}

impl CmdExecutor for RunOpts {
    async fn execute(self) -> anyhow::Result<()> {
        let layer = Layer::new().with_filter(LevelFilter::INFO);
        tracing_subscriber::registry().with(layer).init();
        let (code, config) = get_code_and_config()?;
        let router = SwappableAppRouter::try_new(&code, config.routes)?;
        let pool = SwappableWorkerPool::try_new(code, WOERK_POOL_SIZE)?;
        let routers = vec![TenentRouter::new("localhost", router.clone())];
        let pools = vec![TenentWorkerPool::new("localhost", pool.clone())];
        tokio::spawn(async_watch(".", router, pool));
        start_server(self.port, routers, pools).await?;
        Ok(())
    }
}

fn get_code_and_config() -> anyhow::Result<(String, ProjectConfig)> {
    let (filename, _) = build_project(".")?;
    let config = filename.replace(".mjs", ".yml");
    let code = fs::read_to_string(filename)?;
    let config = ProjectConfig::load(config)?;
    Ok((code, config))
}

async fn async_watch(
    p: &str,
    router: SwappableAppRouter,
    pool: SwappableWorkerPool,
) -> anyhow::Result<()> {
    let (tx, rx) = channel(1);

    let mut debouncer = new_debouncer(MONITOR_FS_INTERVAL, move |res: DebounceEventResult| {
        tx.blocking_send(res).unwrap();
    })?;

    debouncer
        .watcher()
        .watch(p.as_ref(), RecursiveMode::Recursive)?;

    let mut stream = ReceiverStream::new(rx);
    while let Some(ret) = stream.next().await {
        match ret {
            Ok(events) => {
                let mut need_swap = false;
                for event in events {
                    let path = event.path;
                    let ext = path.extension().unwrap_or_default();
                    if path.ends_with("config.yml") || ext == "ts" || ext == "js" {
                        info!("File changed: {}", path.display());
                        need_swap = true;
                        break;
                    }
                }
                if need_swap {
                    let (code, config) = get_code_and_config()?;
                    router.swap(code.clone(), config.routes)?;
                    info!("Router swapped");
                    pool.swap(code)?;
                    info!("Worker Pool swapped");
                }
            }
            Err(e) => {
                warn!("Error: {:?}", e);
            }
        }
    }

    Ok(())
}
