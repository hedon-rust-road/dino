use std::{collections::HashMap, sync::atomic::AtomicUsize, thread};

use axum::{body::Body, response::Response};
use dino_macros::{FromJs, IntoJs};
use rquickjs::{Context, Function, Object, Promise, Runtime};
use tokio::sync::mpsc;
use tracing::info;
use typed_builder::TypedBuilder;

type WorkRequest = (String, Req);
type WorkResponse = oneshot::Sender<Res>;

pub struct JsWorkerPool {
    senders: Vec<mpsc::Sender<(WorkRequest, WorkResponse)>>,
    indexes: AtomicUsize,
}

#[allow(unused)]
pub struct JsWorker {
    rt: Runtime,
    ctx: Context,
}

#[derive(Debug, TypedBuilder, IntoJs)]
pub struct Req {
    #[builder(setter(into))]
    pub method: String,
    #[builder(setter(into))]
    pub url: String,
    #[builder(default)]
    pub query: HashMap<String, String>,
    #[builder(default)]
    pub params: HashMap<String, String>,
    #[builder(default)]
    pub headers: HashMap<String, String>,
    #[builder(default)]
    pub body: Option<String>,
}

#[derive(Debug, FromJs)]
pub struct Res {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl From<Res> for Response {
    fn from(res: Res) -> Self {
        let mut builder = Response::builder().status(res.status);
        for (k, v) in res.headers {
            builder = builder.header(k, v);
        }
        if let Some(body) = res.body {
            builder.body(body.into()).unwrap()
        } else {
            builder.body(Body::empty()).unwrap()
        }
    }
}

fn print(msg: String) {
    println!("hi, here is rust, this is your msg: {msg}")
}

impl JsWorkerPool {
    pub fn new(size: usize, module: &str) -> Self {
        let mut senders = Vec::with_capacity(size);
        for _ in 0..size {
            let (tx, mut rx) = mpsc::channel::<((String, Req), oneshot::Sender<Res>)>(1);
            let code = module.to_string();
            thread::spawn(move || {
                let worker = JsWorker::try_new(&code).unwrap();
                while let Some(((name, req), res_tx)) = rx.blocking_recv() {
                    let res = worker.run(&name, req).unwrap();
                    let _ = res_tx.send(res);
                }
            });
            senders.push(tx);
        }
        Self {
            senders,
            indexes: AtomicUsize::new(0),
        }
    }

    pub async fn run(&self, name: &str, req: Req) -> oneshot::Receiver<Res> {
        let index = self
            .indexes
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let index = index % self.senders.len();
        info!("[worker-{index}] is running {name}");

        let sender = &self.senders[index];
        let (res_tx, res_rx) = oneshot::channel();
        sender
            .send(((name.to_string(), req), res_tx))
            .await
            .unwrap();
        res_rx
    }
}

impl JsWorker {
    pub fn try_new(module: &str) -> anyhow::Result<Self> {
        let rt = Runtime::new()?;
        let ctx = Context::full(&rt)?;

        ctx.with(|ctx| {
            let global = ctx.globals();
            let ret: Object = ctx.eval(module)?;
            global.set("handlers", ret)?;
            // setup print function
            let fun = Function::new(ctx.clone(), print)?.with_name("print")?;
            global.set("print", fun)?;

            Ok::<_, anyhow::Error>(())
        })?;

        Ok(Self { rt, ctx })
    }

    pub fn run(&self, name: &str, req: Req) -> anyhow::Result<Res> {
        self.ctx.with(|ctx| {
            let globals = ctx.globals();
            let handlers = globals.get::<_, Object>("handlers")?;
            let fun = handlers.get::<_, Function>(name)?;
            let v: Promise = fun.call((req,))?;

            Ok::<_, anyhow::Error>(v.finish()?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_worker_should_work() {
        let code = r#"
(function(){
    async function hello(req){
        return {
            status:200,
            headers:{
                "content-type":"application/json"
            },
            body: JSON.stringify(req),
        };
    }
    return{hello:hello};
})();
        "#;

        let req = Req::builder()
            .method("GET")
            .url("https://example.com")
            .headers(HashMap::new())
            .build();

        let worker = JsWorker::try_new(code).unwrap();
        let ret = worker.run("hello", req).unwrap();
        assert_eq!(ret.status, 200);
    }
}
