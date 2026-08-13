use super::*;
use proc_macro2::TokenStream;
use quote::ToTokens;
use std::fs;
use std::path::Path;

#[test]
fn expand() {
    let mut files: Vec<_> = fs::read_dir(Path::new("src/test")).unwrap().filter_map(|e| e.ok()).collect();
    files.retain(|e| {
        e.path().extension().map_or(false, |ext| ext == "rs")
            && !e.file_name().to_string_lossy().ends_with(".expanded.rs")
    });
    files.sort_by_key(|e| e.file_name());
    for file in files {
        let input_path = file.path();
        let input = fs::read_to_string(&input_path).unwrap();
        let file: syn::File = match syn::parse_str(&input) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let has_enum = file.items.iter().any(|item| matches!(item, syn::Item::Enum(_)));
        let has_struct = file.items.iter().any(|item| matches!(item, syn::Item::Struct(_)));
        if has_enum || has_struct {
            let input_tokens: TokenStream = match syn::parse_str(&input) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let expanded = expand_tokens(input_tokens);
            let parsed: syn::File = syn::parse2(expanded).expect(&format!("{input_path:?}"));
            let output = prettyplease::unparse(&parsed);
            let output_path = input_path.with_extension("expanded.rs");
            fs::write(&output_path, &output).unwrap();
        } else {
            panic!("{input_path:?} is invalid");
        }
    }
}

fn expand_tokens(input_tokens: TokenStream) -> TokenStream {
    let file: syn::File = match syn::parse2(input_tokens.clone()) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error(),
    };
    let mut output = TokenStream::new();
    for item in file.items {
        // Always include use statements, type aliases, etc.
        if matches!(&item, syn::Item::Use(_) | syn::Item::ExternCrate(_)) {
            output.extend(item.into_token_stream());
        } else if let syn::Item::Struct(item_struct) = item {
            // Strip #[derive(PcoPack)] and #[pco_pack(...)] so the snapshot is self-contained
            let mut stripped = item_struct.clone();
            strip_pco_pack_attrs(&mut stripped.attrs);
            output.extend(stripped.into_token_stream());
            // Then include the generated derive code
            output.extend(derive_struct_tokens(&item_struct));
        } else if let syn::Item::Enum(item_enum) = item {
            // Strip #[derive(PcoPack)] and #[pco_pack(...)] so the snapshot is self-contained
            let mut stripped = item_enum.clone();
            strip_pco_pack_attrs(&mut stripped.attrs);
            output.extend(stripped.into_token_stream());
            // Then include the generated derive code
            output.extend(derive_enum_tokens(&item_enum));
        }
    }
    output
}

fn strip_pco_pack_attrs(attrs: &mut Vec<syn::Attribute>) {
    attrs.retain(|attr| !attr.path().is_ident("pco_pack"));

    // Strip PcoPack from any #[derive(PcoPack, ...)] attribute lists
    for attr in attrs.iter_mut() {
        if let syn::Meta::List(meta_list) = &mut attr.meta {
            meta_list.tokens = meta_list
                .tokens
                .clone()
                .into_iter()
                .filter(|tt| !matches!(tt, proc_macro2::TokenTree::Ident(i) if i == "PcoPack"))
                .collect();
        }
    }

    // Remove empty #[derive()] attributes left after stripping PcoPack
    attrs.retain(|attr| {
        !(attr.path().is_ident("derive") && attr.meta.require_list().ok().map_or(false, |list| list.tokens.is_empty()))
    });
}

fn derive_struct_tokens(input: &syn::ItemStruct) -> TokenStream {
    let name = &input.ident;
    let attrs = match parse_pco_pack_attrs(&input.attrs) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error(),
    };
    match &input.fields {
        syn::Fields::Named(fields) => r#struct::generate(name, &fields.named, &attrs),
        _ => syn::Error::new_spanned(&input.ident, "PcoPack requires named fields").to_compile_error(),
    }
}

fn derive_enum_tokens(input: &syn::ItemEnum) -> TokenStream {
    let name = &input.ident;
    let attrs = PcoPackAttrs::default();
    if input.variants.is_empty() {
        syn::Error::new_spanned(&input.ident, "PcoPack cannot be derived for empty enums").to_compile_error()
    } else {
        r#enum::generate_enum_tokens(name, &input.variants, &attrs)
    }
}
