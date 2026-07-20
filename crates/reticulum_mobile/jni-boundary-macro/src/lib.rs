use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, parse_quote, ItemFn};

/// Wrap a JNI export in the crate's panic containment helper.
///
/// The operation name is derived from the Java export suffix so every panic is
/// reported through the same last-error envelope as ordinary native failures.
#[proc_macro_attribute]
pub fn jni_boundary(_attribute: TokenStream, item: TokenStream) -> TokenStream {
    let mut function = parse_macro_input!(item as ItemFn);
    let function_name = function.sig.ident.to_string();
    let operation = function_name
        .rsplit_once("ReticulumBridge_")
        .map_or(function_name.as_str(), |(_, suffix)| suffix);
    let operation = syn::LitStr::new(operation, function.sig.ident.span());
    let original_body = function.block;

    function.block = Box::new(parse_quote!({
        contain_jni_panic(#operation, || #original_body)
    }));

    quote!(#function).into()
}
