use std::collections::HashMap;

use rquickjs::{Context, FromJs, Function, IntoJs, Object, Promise, Runtime};
use typed_builder::TypedBuilder;

#[allow(unused)]
pub struct JsWorker {
    rt: Runtime,
    ctx: Context,
}

#[derive(Debug, TypedBuilder)]
pub struct Request {
    pub headers: HashMap<String, String>,
    #[builder(setter(into))]
    pub method: String,
    #[builder(setter(into))]
    pub url: String,
    #[builder(default, setter(strip_option))]
    pub body: Option<String>,
}

#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

fn print(msg: String) {
    println!("hi, here is rust, this is your msg: {msg}")
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

    pub fn run(&self, name: &str, req: Request) -> anyhow::Result<Response> {
        self.ctx.with(|ctx| {
            let globals = ctx.globals();
            let handlers = globals.get::<_, Object>("handlers")?;
            let fun = handlers.get::<_, Function>(name)?;
            let v: Promise = fun.call((req,))?;

            Ok::<_, anyhow::Error>(v.finish()?)
        })
    }
}

impl<'js> IntoJs<'js> for Request {
    fn into_js(self, ctx: &rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        let obj = ctx.globals();
        obj.set("headers", self.headers)?;
        obj.set("method", self.method)?;
        obj.set("url", self.url)?;
        obj.set("body", self.body)?;
        Ok(obj.into())
    }
}

impl<'js> FromJs<'js> for Response {
    fn from_js(_ctx: &rquickjs::Ctx<'js>, v: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
        let obj = v.into_object().unwrap();
        let status = obj.get::<_, u16>("status")?;
        let headers = obj.get::<_, HashMap<String, String>>("headers")?;
        let body = obj.get::<_, Option<String>>("body")?;
        Ok(Self {
            status,
            headers,
            body,
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

        let req = Request::builder()
            .method("GET")
            .url("https://example.com")
            .headers(HashMap::new())
            .build();

        let worker = JsWorker::try_new(code).unwrap();
        let ret = worker.run("hello", req).unwrap();
        assert_eq!(ret.status, 200);
    }
}
