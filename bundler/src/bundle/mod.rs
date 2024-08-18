mod modules;
mod transpilers;

use self::modules::load_import;
use self::modules::resolve_import;
use self::modules::ImportMap;
use self::modules::CORE_MODULES;
use anyhow::Error;
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use swc_atoms::js_word;
use swc_atoms::JsWord;
use swc_bundler::Bundler;
use swc_bundler::Config;
use swc_bundler::Load;
use swc_bundler::ModuleData;
use swc_bundler::ModuleRecord;
use swc_bundler::Resolve;
use swc_common::errors::ColorConfig;
use swc_common::errors::Handler;
use swc_common::source_map::SourceMap;
use swc_common::sync::Lrc;
use swc_common::FileName;
use swc_common::FilePathMapping;
use swc_common::Globals;
use swc_common::Span;
use swc_ecma_ast::*;
use swc_ecma_codegen::text_writer::JsWriter;
use swc_ecma_codegen::Emitter;
use swc_ecma_loader::resolve::Resolution;
use swc_ecma_parser::parse_file_as_module;
use swc_ecma_parser::EsSyntax;
use swc_ecma_parser::Syntax;

#[derive(Debug, Default, Clone)]
pub struct Options {
    pub skip_cache: bool,
    pub minify: bool,
    pub import_map: Option<ImportMap>,
}

pub fn run_bundle(entry: &str, options: &Options) -> Result<String> {
    // Create SWC globals and an LRC sourcemap.
    let globals = Globals::default();
    let cm = Lrc::new(SourceMap::new(FilePathMapping::empty()));

    // NOTE: Core modules are built-in to dune's binary so there is no point to pollute
    // the bundle with extra code that the runtime can load anyway.
    let external_modules: Vec<JsWord> = CORE_MODULES.keys().map(|k| (*k).into()).collect();

    // Create the bundler.
    let mut bundler = Bundler::new(
        &globals,
        cm.clone(),
        Loader {
            cm: cm.clone(),
            options,
        },
        Resolver { options },
        Config {
            external_modules,
            require: false,
            ..Default::default()
        },
        Box::new(Hook),
    );

    // Create bundle entries.
    let mut entries = HashMap::default();
    entries.insert("main".to_string(), FileName::Real(entry.into()));

    // Bundle entries.
    let bundle = bundler
        .bundle(entries)
        .map_err(|e| Error::msg(format!("{e:?}")))?
        .pop()
        .unwrap();

    let mut buf = vec![];

    {
        let mut cfg = swc_ecma_codegen::Config::default();
        cfg.minify = options.minify;

        let mut emitter = Emitter {
            cfg,
            cm: cm.clone(),
            comments: None,
            wr: Box::new(JsWriter::new(cm, "\n", &mut buf, None)),
        };

        emitter.emit_module(&bundle.module)?;
    }

    // Build source from bytes.
    let mut source = String::from_utf8(buf).unwrap();

    if !options.minify {
        // Decorate output with the following messages.
        let messages = [
            format!("// Dune v{}\n", env!("CARGO_PKG_VERSION")),
            "// It's not recommended to edit this code manually since it's generated by `dune bundle`\n\n".into()
        ];
        messages.iter().rev().for_each(|msg| {
            source.insert_str(0, msg);
        });
    }

    Ok(source)
}

struct Loader<'s> {
    cm: Lrc<SourceMap>,
    options: &'s Options,
}

impl<'s> Load for Loader<'s> {
    fn load(&self, file: &FileName) -> Result<ModuleData, Error> {
        // We only dealing with `Real` filenames.
        let specifier = match file {
            FileName::Real(value) => value.to_string_lossy().to_string(),
            _ => unreachable!(),
        };

        // Try load the module's source-code.
        let source = load_import(&specifier, self.options.skip_cache)?;
        let path = FileName::Real(specifier.into());
        let fm = self.cm.new_source_file(path, source);

        let handler =
            Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(self.cm.clone()));

        // Parse JavaScript source into an SWC module.
        let module = match parse_file_as_module(
            &fm,
            Syntax::Es(EsSyntax::default()),
            EsVersion::latest(),
            None,
            &mut vec![],
        )
        .map_err(|e| e.into_diagnostic(&handler).emit())
        {
            Ok(module) => module,
            Err(_) => std::process::exit(1),
        };

        Ok(ModuleData {
            fm,
            module,
            helpers: Default::default(),
        })
    }
}

struct Resolver<'a> {
    options: &'a Options,
}

impl<'a> Resolve for Resolver<'a> {
    fn resolve(&self, base: &FileName, specifier: &str) -> Result<Resolution, Error> {
        // We only dealing with `Real` filenames.
        let base = match base {
            FileName::Real(value) => value.to_str(),
            _ => unreachable!(),
        };

        // Try resolve the specifier.
        Ok(Resolution {
            filename: FileName::Real(
                Path::new(&resolve_import(
                    base,
                    specifier,
                    true,
                    self.options.import_map.clone(),
                )?)
                .to_path_buf(),
            ),
            slug: None,
        })
    }
}

struct Hook;

impl swc_bundler::Hook for Hook {
    fn get_import_meta_props(
        &self,
        span: Span,
        module: &ModuleRecord,
    ) -> Result<Vec<KeyValueProp>, Error> {
        // Get filename as string.
        let file_name = module.file_name.to_string();
        let file_name = resolve_import(None, &file_name, true, None)?;

        // Compute .main and .url properties.
        Ok(vec![
            KeyValueProp {
                key: PropName::Ident(Ident::new(js_word!("url"), span)),
                value: Box::new(Expr::Lit(Lit::Str(Str {
                    span,
                    raw: None,
                    value: file_name.into(),
                }))),
            },
            KeyValueProp {
                key: PropName::Ident(Ident::new(js_word!("main"), span)),
                value: Box::new(if module.is_entry {
                    Expr::Member(MemberExpr {
                        span,
                        obj: Box::new(Expr::MetaProp(MetaPropExpr {
                            span,
                            kind: MetaPropKind::ImportMeta,
                        })),
                        prop: MemberProp::Ident(Ident::new(js_word!("main"), span)),
                    })
                } else {
                    Expr::Lit(Lit::Bool(Bool { span, value: false }))
                }),
            },
        ])
    }
}
