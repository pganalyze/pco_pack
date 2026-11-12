use super::*;
use proc_macro2::{Group, TokenStream, TokenTree};
use quote::ToTokens;
use std::fs;
use std::path::Path;

#[test]
fn expand() {
    let mut files: Vec<_> = fs::read_dir(Path::new("src/test")).unwrap().filter_map(|e| e.ok()).collect();
    files.retain(|e| {
        e.path().extension().is_some_and(|ext| ext == "rs")
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
            let parsed: syn::File = syn::parse2(expanded).unwrap_or_else(|_| panic!("{input_path:?}"));
            let output = prettyplease::unparse(&parsed);
            let output_path = input_path.with_extension("expanded.rs");
            fs::write(&output_path, &output).unwrap();
        } else {
            panic!("{input_path:?} is invalid");
        }
    }
}

#[test]
fn float_index_field_rejected() {
    let expand_indexed = |ty: &str| -> String {
        let item: syn::ItemStruct = syn::parse_str(&format!(
            "#[derive(PcoPack)] #[pco_pack(index = [ratio])] struct S {{ ratio: {ty}, value: i64 }}"
        ))
        .unwrap();
        derive_struct_tokens(&item).to_string()
    };

    for ty in ["f32", "f64", "half::f16", "Option<f64>", "Vec<Option<f32>>"] {
        let expanded = expand_indexed(ty);
        assert!(expanded.contains("compile_error"), "{ty} index field unexpectedly expanded to:\n{expanded}");
    }

    let expanded = expand_indexed("Option<i64>");
    assert!(!expanded.contains("compile_error"), "Option<i64> index field was rejected:\n{expanded}");
}

/// Expands a valid-Rust input file the way the compiler would: derive expansions are
/// appended after their (stripped) item, and `fold!` invocations are replaced in place
/// with the block the macro emits at that position.
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
        } else {
            // Any other item (fn, const, ...) carries through unchanged apart from its
            // fold! invocations expanding to the blocks the macro would emit there.
            output.extend(expand_fold_calls(item.into_token_stream()));
        }
    }
    output
}

/// Rewrites `tokens` the way the compiler would, replacing every `fold!` invocation,
/// bare or path-qualified, with the macro's expansion, descending into delimited groups
/// so nested calls inside fn bodies, initializers, and similar positions get expanded.
fn expand_fold_calls(tokens: TokenStream) -> TokenStream {
    let tts: Vec<TokenTree> = tokens.into_iter().collect();
    let mut output: Vec<TokenTree> = Vec::with_capacity(tts.len());
    let mut i = 0;
    while i < tts.len() {
        let is_fold = matches!(&tts[i], TokenTree::Ident(id) if id == "fold")
            && matches!(tts.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == '!')
            && matches!(tts.get(i + 2), Some(TokenTree::Group(_)));
        if is_fold {
            let TokenTree::Group(args) = &tts[i + 2] else { unreachable!("matched above") };
            // A qualified call carries its path in the already-emitted tokens
            // (`pco_pack` `::` precede `fold`), and rustc consumes all of it.
            while output.len() >= 2
                && matches!(&output[output.len() - 2..],
                    [TokenTree::Punct(a), TokenTree::Punct(b)] if a.as_char() == ':' && b.as_char() == ':')
            {
                output.truncate(output.len() - 2);
                if matches!(output.last(), Some(TokenTree::Ident(_))) {
                    output.pop();
                }
            }
            let expansion = match syn::parse2::<fold::FoldInput>(args.stream()).and_then(fold::expand) {
                Ok(v) => v,
                Err(e) => return e.to_compile_error(),
            };
            // fold::expand emits a single brace block, exactly the token the call site
            // is replaced with, so splice it in as-is.
            output.extend(expansion);
            i += 3;
        } else if let TokenTree::Group(group) = &tts[i] {
            let mut rewritten = Group::new(group.delimiter(), expand_fold_calls(group.stream()));
            rewritten.set_span(group.span());
            output.push(TokenTree::Group(rewritten));
            i += 1;
        } else {
            output.push(tts[i].clone());
            i += 1;
        }
    }
    output.into_iter().collect()
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
        !(attr.path().is_ident("derive") && attr.meta.require_list().ok().is_some_and(|list| list.tokens.is_empty()))
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
    let attrs = match parse_pco_pack_attrs(&input.attrs) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error(),
    };
    if input.variants.is_empty() {
        syn::Error::new_spanned(&input.ident, "PcoPack cannot be derived for empty enums").to_compile_error()
    } else {
        r#enum::generate_enum_tokens(name, &input.variants, &attrs)
    }
}
