use super::parse::{FieldRole, StructGen};
use super::type_helpers::{extract_timeline_const_generic, is_timeline_type};
use quote::quote;

/// Generate the struct definitions: wrapper (Chunk), group (Group), reader (Reader), and writer (Writer).
/// Also generates Clone/Default impls and `with_fields` methods.
pub fn generate(sg: &StructGen) -> proc_macro2::TokenStream {
    let has_timestamp = sg.has_timestamp;

    // Wrapper struct fields: index raw fields, start_at/end_at i64, payload ByteBufs
    let mut chunk_fields: Vec<proc_macro2::TokenStream> = sg
        .index_fields()
        .iter()
        .map(|fi| {
            let ident = &fi.ident;
            let ty = &fi.ty;
            quote! { pub #ident: #ty, }
        })
        .collect();
    if has_timestamp {
        chunk_fields.push(quote! {
            #[serde(with = "pco_pack::chrono::serde::ts_microseconds")]
            pub start_at: pco_pack::chrono::DateTime<pco_pack::chrono::Utc>,
        });
        chunk_fields.push(quote! {
            #[serde(with = "pco_pack::chrono::serde::ts_microseconds")]
            pub end_at: pco_pack::chrono::DateTime<pco_pack::chrono::Utc>,
        });
    }
    for fi in &sg.field_infos {
        if !matches!(fi.role, FieldRole::Index) && !matches!(fi.role, FieldRole::Timestamp) {
            let ident = &fi.ident;
            chunk_fields.push(quote! { #[serde(default)] pub #ident: serde_bytes::ByteBuf, });
        }
    }
    if let Some(rf) = sg.timestamp_field() {
        let ident = &rf.ident;
        chunk_fields.push(quote! { #[serde(default)] pub #ident: serde_bytes::ByteBuf, });
    }

    // Group struct fields: index raw, timestamp/payload uses LazyReader, start_at/end_at i64
    let mut reader_fields: Vec<_> = sg
        .field_infos
        .iter()
        .map(|fi| {
            let ident = &fi.ident;
            match fi.role {
                FieldRole::Index => {
                    let ty = &fi.ty;
                    quote! { pub #ident: #ty, }
                }
                _ => {
                    let ty = &fi.ty;
                    quote! { pub #ident: pco_pack::LazyReader<#ty>, }
                }
            }
        })
        .collect();
    if has_timestamp {
        reader_fields.push(quote! {
            pub start_at: pco_pack::chrono::DateTime<pco_pack::chrono::Utc>,
        });
        reader_fields.push(quote! {
            pub end_at: pco_pack::chrono::DateTime<pco_pack::chrono::Utc>,
        });
    }

    let writer_fields: Vec<_> = sg
        .field_infos
        .iter()
        .map(|fi| {
            let ident = &fi.ident;
            let ct = if fi.role == FieldRole::Timestamp {
                if is_timeline_type(&fi.ty) {
                    let const_generic = extract_timeline_const_generic(&fi.ty);
                    quote! { pco_pack::Timeline<#const_generic> }
                } else {
                    quote! { i64 }
                }
            } else {
                let ty = &fi.ty;
                quote! { #ty }
            };
            quote! { pub #ident: Vec<#ct>, }
        })
        .collect();

    // Column field names (for Default/Clone impls)
    let cols_field_names: Vec<proc_macro2::TokenStream> = sg
        .field_infos
        .iter()
        .map(|fi| {
            let ident = &fi.ident;
            quote! { #ident }
        })
        .collect();

    // Group::with_fields: keep index and timestamp fields (always required), replace payload
    // fields with empty LazyReader if not in `fields`.
    let decimals = sg.float_round.unwrap_or(0);
    let time_round = sg.time_round.as_ref().map(|tr| quote! { #tr }).unwrap_or(quote! { Default::default() });
    let mut with_fields: Vec<proc_macro2::TokenStream> = sg
        .field_infos
        .iter()
        .map(|fi| {
            let ident = &fi.ident;
            let field_name = ident.to_string();
            match fi.role {
                FieldRole::Index => {
                    // Index fields are required and always decompressed.
                    quote! { #ident: self.#ident.clone(), }
                }
                _ => {
                    let ty = &fi.ty;
                    quote! {
                        #ident: <pco_pack::LazyReader<#ty>>::new(
                            if fields.is_empty() || fields.contains(&#field_name) {
                                self.#ident.to_vec()
                            } else {
                                Vec::new()
                            }
                            , #decimals, #time_round
                        ),
                    }
                }
            }
        })
        .collect();
    if has_timestamp {
        with_fields.push(quote! { start_at: self.start_at.clone(), });
        with_fields.push(quote! { end_at: self.end_at.clone(), });
    }

    let wrapper_doc = format!(" Intermediate compressed form for [{}] with metadata fields", sg.name);
    quote! {
        #[doc = #wrapper_doc]
        /// (index, timestamp bounds) uncompressed and payload columns
        /// stored as compressed ByteBuf. One instance per logical group.
        #[derive(pco_pack::serde::Serialize, pco_pack::serde::Deserialize, Default, Clone)]
        pub struct Chunk { #(#chunk_fields)* }
        #[derive(Clone, Default)]
        pub struct Reader { #(#reader_fields)* }
        pub struct Writer { #(#writer_fields)* }
        impl Default for Writer {
            fn default() -> Self { Self { #(#cols_field_names: Default::default(),)* } }
        }
    }
}
