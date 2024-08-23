use darling::{
    ast::{Data, Style},
    FromDeriveInput, FromField,
};
use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(error_info))]
struct StructData {
    ident: syn::Ident,
    generics: syn::Generics,
    data: Data<(), StructFields>,
}

#[derive(Debug, FromField)]
struct StructFields {
    ident: Option<syn::Ident>,
    ty: syn::Type,
}

pub(crate) fn process_from_js(input: DeriveInput) -> TokenStream {
    let (ident, generics, merged, fields) = parse_struct(input);

    let code = fields.iter().map(|field| {
        let name = field.ident.as_ref().expect("Field must be named");
        let ty = &field.ty;

        quote! {
            let #name = obj.get::<_, #ty>(stringify!(#name))?;
        }
    });

    let idents = fields.iter().map(|field| {
        let name = field.ident.as_ref().expect("Field must be named");
        quote! {#name}
    });

    quote! {
        impl #merged rquickjs::FromJs<'js> for #ident #generics {
            fn from_js(_ctx: &rquickjs::Ctx<'js>, v: rquickjs::Value<'js>) -> rquickjs::Result<Self> {
                let obj = v.into_object().unwrap();

                #(#code)*

                Ok(Self {
                    #(#idents),*
                })
            }
        }
    }

    /*
     impl<'js> rquickjs::FromJs<'js> for Response {
        fn from_js(
            _ctx: &rquickjs::Ctx<'js>,
            v: rquickjs::Value<'js>,
        ) -> rquickjs::Result<Self> {
            let obj = v.into_object().unwrap();
            let status = obj.get::<_, u16>("#name")?;
            let headers = obj.get::<_, HashMap<String, String>>("#name")?;
            let body = obj.get::<_, Option<String>>("#name")?;
            Ok(Self { status, headers, body })
        }
    }
     */
}

pub(crate) fn process_into_js(input: DeriveInput) -> TokenStream {
    let (ident, generics, merged, fields) = parse_struct(input);

    let code = fields.iter().map(|field| {
        let name = field.ident.as_ref().expect("Field must be named");
        quote! {
            obj.set(stringify!(#name), self.#name)?;
        }
    });

    quote! {
        impl #merged rquickjs::IntoJs<'js> for #ident #generics {
            fn into_js(self, ctx: &rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                let obj = ctx.globals();
                #(#code)*
                Ok(obj.into())
            }
        }
    }

    /*
    impl<'js> rquickjs::IntoJs<'js> for Request {
        fn into_js(self, ctx: &rquickjs::Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
            let obj = ctx.globals();
            obj.set("headers", self.headers)?;
            obj.set("method", self.method)?;
            obj.set("url", self.url)?;
            obj.set("body", self.body)?;
            Ok(obj.into())
        }
    }
     */
}

fn parse_struct(
    input: DeriveInput,
) -> (syn::Ident, syn::Generics, syn::Generics, Vec<StructFields>) {
    let StructData {
        ident,
        generics,
        data: Data::Struct(fields),
    } = StructData::from_derive_input(&input).expect("can not parse input")
    else {
        panic!("Only struct is supported")
    };

    let fields = match fields.style {
        Style::Struct => fields.fields,
        _ => panic!("Only named fields are supported"),
    };

    let mut merged = generics.clone();
    merged.params.push(syn::parse_quote!('js));

    (ident, generics, merged, fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_from_js_should_work() {
        let input = r#"
        #[derive(IntoJs)]
          struct Request {
            method: String,
            url: String,
            headers: HashMap<String, String>,
            body: Option<String>,
          }
        "#;

        let parsed = syn::parse_str(input).unwrap();
        let info = StructData::from_derive_input(&parsed).unwrap();

        assert_eq!(info.ident.to_string(), "Request");

        let code = process_from_js(parsed);
        println!("{}", code);
    }

    #[test]
    fn process_into_js_should_work() {
        let input = r#"
        #[derive(IntoJs)]
          struct Response {
            status: u16,
            headers: HashMap<String, String>,
            body: Option<String>,
          }
        "#;

        let parsed = syn::parse_str(input).unwrap();
        let info = StructData::from_derive_input(&parsed).unwrap();

        assert_eq!(info.ident.to_string(), "Response");

        let code = process_into_js(parsed);
        println!("{}", code);
    }
}
