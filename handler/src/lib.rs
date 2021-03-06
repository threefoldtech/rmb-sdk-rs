use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, ItemFn};

#[proc_macro_attribute]
pub fn handler(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemFn);

    if input.sig.asyncness.is_none() {
        panic!("supported only for async functions");
    }

    let vis = &input.vis;
    let args = &input.sig.inputs;
    if args.len() != 2 {
        panic!("handler must accept two arguments (D, HandlerInput)");
    }

    let data = if let Some(FnArg::Typed(ref ty)) = args.first() {
        ty
    } else {
        panic!("app data type missing");
    };

    let p = &data.ty;

    let name = &input.sig.ident;
    let out = quote! {

        #[allow(non_camel_case_types)]
        #vis struct #name;

        #[async_trait::async_trait]
        impl Handler<#p> for #name
        {
            async fn call(&self, data: #p, input: HandlerInput) -> Result<HandlerOutput> {
                #input

                #name(data, input).await
            }
        }

    };

    TokenStream::from(out)
}
