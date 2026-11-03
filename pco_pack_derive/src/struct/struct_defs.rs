use super::parse::{FieldRole, StructGen};
use super::type_helpers::{is_copy_scalar, is_timeline_type};
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
            chunk_fields.push(quote! { #[serde(default)] pub #ident: pco_pack::serde_bytes::ByteBuf, });
        }
    }
    if let Some(rf) = sg.timestamp_field() {
        let ident = &rf.ident;
        chunk_fields.push(quote! { #[serde(default)] pub #ident: pco_pack::serde_bytes::ByteBuf, });
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
                    quote! { pco_pack::Timeline }
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

    // Columns view accessors: index fields come from uncompressed chunk metadata, payload
    // and timestamp fields pop through their LazyReader (same semantics as expand_row).
    let columns_accessors: Vec<proc_macro2::TokenStream> = sg
        .field_infos
        .iter()
        .map(|fi| {
            let ident = &fi.ident;
            let ty = &fi.ty;
            if fi.role == FieldRole::Index {
                let read = if is_copy_scalar(&fi.ty) {
                    quote! { Ok(self.reader.#ident) }
                } else {
                    quote! { Ok(self.reader.#ident.clone()) }
                };
                quote! {
                    pub fn #ident(&self, _row: usize) -> pco_pack::anyhow::Result<#ty> { #read }
                }
            } else {
                quote! {
                    pub fn #ident(&mut self, row: usize) -> pco_pack::anyhow::Result<#ty> {
                        self.reader.#ident.pop_inner(row)
                    }
                }
            }
        })
        .collect();

    quote! {
        #[doc = #wrapper_doc]
        /// (index, timestamp bounds) uncompressed and payload columns
        /// stored as compressed ByteBuf. One instance per logical group.
        #[derive(pco_pack::serde::Serialize, pco_pack::serde::Deserialize, Default, Clone)]
        #[serde(crate = "pco_pack::serde")]
        pub struct Chunk { #(#chunk_fields)* }
        #[derive(Clone, Default)]
        pub struct Reader { #(#reader_fields)* }
        #[derive(Default)]
        pub struct Writer { #(#writer_fields)* }

        /// Typed view over one chunk's decompressed columns, with one accessor per field.
        /// Fields outside the loaded projection return `Default`, matching `filter` semantics.
        /// Rows are read in ascending order only, mirroring the forward-only `PcoSerde::get`.
        /// Chunk bounds (`start_at`/`end_at`) have no accessors; they reach consumers only
        /// through `hydrate` rows, never through `fold!` projections.
        pub struct Columns<'a> {
            reader: &'a mut Reader,
        }

        impl Columns<'_> {
            #(#columns_accessors)*
        }
    }
}
