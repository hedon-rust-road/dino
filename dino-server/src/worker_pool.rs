use std::{ops::Deref, sync::Arc};

use arc_swap::ArcSwap;

use crate::{JsWorkerPool, Req, Res};

#[derive(Clone)]
pub struct SwappableWorkerPool {
    pub size: usize,
    pub inner: Arc<ArcSwap<WorkerPoolInner>>,
}

pub struct WorkerPoolInner {
    pub code: String,
    pub pool: JsWorkerPool,
}

#[derive(Clone)]
pub struct WorkerPool(Arc<WorkerPoolInner>);

impl SwappableWorkerPool {
    pub fn try_new(code: impl Into<String>, size: usize) -> anyhow::Result<Self> {
        let code = code.into();
        let pool = JsWorkerPool::new(size, &code);
        let inner = WorkerPoolInner::new(code, pool);
        Ok(Self {
            size,
            inner: Arc::new(ArcSwap::from_pointee(inner)),
        })
    }

    pub fn swap(&self, code: impl Into<String>) -> anyhow::Result<()> {
        let code = code.into();
        let pool = JsWorkerPool::new(self.size, &code);
        let inner = WorkerPoolInner::new(code, pool);
        self.inner.store(Arc::new(inner));
        Ok(())
    }

    pub fn load(&self) -> WorkerPool {
        WorkerPool(self.inner.load_full())
    }
}

impl Deref for WorkerPool {
    type Target = WorkerPoolInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl WorkerPoolInner {
    pub fn new(code: impl Into<String>, pool: JsWorkerPool) -> Self {
        Self {
            code: code.into(),
            pool,
        }
    }

    pub async fn run(&self, name: &str, req: Req) -> anyhow::Result<Res> {
        let rx = self.pool.run(name, req).await;
        Ok(rx.recv()?)
    }
}
