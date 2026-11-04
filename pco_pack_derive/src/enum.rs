use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Fields, Ident, Variant};

use crate::PcoPackAttrs;

/// Represents an enum variant with its shape information.
struct EnumVariantInfo {
    ident: Ident,
    discriminant: i64,
    is_unit: bool,
    /// For tuple variants: the type of the single payload field.
    payload_ty: Option<syn::Type>,
}

fn parse_discriminant_expr(expr: &syn::Expr) -> Option<i64> {
    if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(int), .. }) = expr {
        int.base10_parse().ok()
    } else if let syn::Expr::Unary(syn::ExprUnary { op: syn::UnOp::Neg(_), expr: inner, .. }) = expr {
        if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(int), .. }) = inner.as_ref() {
            int.base10_parse::<i64>().ok().map(|v| -v)
        } else {
            None
        }
    } else {
        None
    }
}

pub fn generate_enum_tokens(
    name: &syn::Ident, variants: &syn::punctuated::Punctuated<Variant, syn::token::Comma>, attrs: &PcoPackAttrs,
) -> TokenStream2 {
    let mut variant_infos = Vec::new();
    let mut has_explicit_first: bool = false;
    let mut prev_discriminant: i64 = 0;
    for (idx, variant) in variants.iter().enumerate() {
        let discriminant = if let Some((_, expr)) = &variant.discriminant {
            if let Some(val) = parse_discriminant_expr(expr) { val } else { prev_discriminant + 1 }
        } else {
            if !has_explicit_first && idx == 0 { 0 } else { prev_discriminant + 1 }
        };
        if variant.discriminant.is_some() {
            has_explicit_first = true;
        }
        prev_discriminant = discriminant;

        match &variant.fields {
            Fields::Unit => {
                variant_infos.push(EnumVariantInfo {
                    ident: variant.ident.clone(),
                    discriminant,
                    is_unit: true,
                    payload_ty: None,
                });
            }
            Fields::Unnamed(fields) => {
                if fields.unnamed.len() != 1 {
                    return syn::Error::new(
                        variant.ident.span(),
                        format!(
                            "PcoPack enum variant {} (discriminant {}) must be a unit variant or have exactly one field",
                            variant.ident, discriminant
                        ),
                    )
                    .to_compile_error();
                }
                let payload_ty = fields.unnamed.first().unwrap().ty.clone();
                variant_infos.push(EnumVariantInfo {
                    ident: variant.ident.clone(),
                    discriminant,
                    is_unit: false,
                    payload_ty: Some(payload_ty),
                });
            }
            Fields::Named(_) => {
                return syn::Error::new(
                    variant.ident.span(),
                    format!(
                        "PcoPack enum variant {} (discriminant {}) must be a unit variant or a single-field tuple variant",
                        variant.ident, discriminant
                    ),
                )
                .to_compile_error();
            }
        }
    }

    let payload_field_ids: Vec<syn::Ident> = variant_infos
        .iter()
        .filter(|v| !v.is_unit)
        .map(|v| syn::Ident::new(&format!("variant_{}", v.discriminant), v.ident.span()))
        .collect();

    // Existence markers per variant, referenced by `fold!` patterns for compile-time
    // validation (emitted after the impls below).
    let variant_probe_idents: Vec<syn::Ident> =
        variant_infos.iter().map(|v| syn::Ident::new(&format!("__pco_variant_{}", v.ident), v.ident.span())).collect();

    // Payload types as TokenStreams for struct field declarations
    let payload_field_tys_tokens: Vec<proc_macro2::TokenStream> =
        variant_infos.iter().filter_map(|v| v.payload_ty.as_ref().map(|ty| quote!(#ty))).collect();

    let struct_field_decls: proc_macro2::TokenStream = payload_field_ids
        .iter()
        .zip(payload_field_tys_tokens.iter())
        .map(|(field_id, ty_token)| {
            quote! { #field_id: Vec<#ty_token>, }
        })
        .collect();

    let non_unit_count: usize = payload_field_tys_tokens.len();

    let reader_field_decls: proc_macro2::TokenStream = payload_field_ids
        .iter()
        .zip(payload_field_tys_tokens.iter())
        .map(|(field_id, ty_token)| {
            quote! { #field_id: pco_pack::LazyReader<#ty_token>, }
        })
        .collect();

    let reader_variant_index_decls: proc_macro2::TokenStream = payload_field_ids
        .iter()
        .map(|field_id| {
            let index_name = syn::Ident::new(&format!("{}_index", field_id), field_id.span());
            let last_row = syn::Ident::new(&format!("{}_last_row", field_id), field_id.span());
            quote! {
                #index_name: usize,
                #[cfg(debug_assertions)]
                #last_row: Option<usize>,
            }
        })
        .collect();

    let field_names_only: proc_macro2::TokenStream = payload_field_ids
        .iter()
        .map(|field_id| {
            let index_name = syn::Ident::new(&format!("{}_index", field_id), field_id.span());
            let last_row = syn::Ident::new(&format!("{}_last_row", field_id), field_id.span());
            quote! {
                #field_id,
                #index_name: 0,
                #[cfg(debug_assertions)]
                #last_row: None,
            }
        })
        .collect();

    let serialize_arms: proc_macro2::TokenStream = variant_infos
        .iter()
        .map(|v| {
            let variant_ident = &v.ident;
            let discriminant = v.discriminant;

            if v.is_unit {
                quote! {
                    Self::#variant_ident => {
                        writer.row_variant.values.push(#discriminant);
                    }
                }
            } else {
                let payload_field = syn::Ident::new(&format!("variant_{}", discriminant), v.ident.span());
                _ = v.payload_ty.as_ref();
                quote! {
                    Self::#variant_ident(inner) => {
                        writer.row_variant.values.push(#discriminant);
                        writer.#payload_field.push(inner);
                    }
                }
            }
        })
        .collect();

    // A discriminant outside the variant table means corruption or an incompatible
    // schema, never schema evolution: error instead of silently hydrating damaged rows
    // as the default variant.
    let get_arms: proc_macro2::TokenStream = variant_infos
        .iter()
        .map(|v| {
            let variant_ident = &v.ident;
            let discriminant = v.discriminant;

            if v.is_unit {
                quote! {
                    Some(#discriminant) => Ok(Some(Self::#variant_ident)),
                }
            } else {
                let payload_field = syn::Ident::new(&format!("variant_{}", discriminant), v.ident.span());
                let index_name = syn::Ident::new(&format!("{}_index", payload_field), payload_field.span());
                quote! {
                    Some(#discriminant) => {{
                        let idx = reader.#index_name;
                        reader.#index_name += 1;
                        Ok(Some(Self::#variant_ident(reader.#payload_field.pop_inner(idx)?)))
                    }},
                }
            }
        })
        .collect();

    let matches_arms: proc_macro2::TokenStream = variant_infos
        .iter()
        .map(|v| {
            let variant_ident = &v.ident;
            let discriminant = v.discriminant;
            if v.is_unit {
                quote! {
                    Self::#variant_ident => match filter {
                        pco_pack::Filter::I64(target) => *target == #discriminant,
                        _ => false,
                    },
                }
            } else {
                quote! {
                    Self::#variant_ident(_inner) => {
                        match filter {
                            pco_pack::Filter::I64(target) => *target == #discriminant,
                            pco_pack::Filter::InclusionI64(targets) => targets.contains(&#discriminant),
                            _ => false,
                        }
                    }
                }
            }
        })
        .collect();

    let variant_serialize_blocks: proc_macro2::TokenStream = variant_infos
        .iter()
        .filter(|v| !v.is_unit)
        .map(|v| {
            let field_id = syn::Ident::new(&format!("variant_{}", v.discriminant), v.ident.span());
            let payload_ty = v.payload_ty.as_ref().unwrap();
            quote! {
                if !writer.#field_id.is_empty() {
                    let compressed = <#payload_ty as pco_pack::PcoSerde>::write(writer.#field_id, float_round, time_round)?;
                    out.extend_from_slice(&(compressed.len() as u64).to_le_bytes());
                    out.extend_from_slice(&compressed);
                } else {
                    out.extend_from_slice(&(0u64).to_le_bytes());
                }
            }
        })
        .collect();

    let non_unit_variants: Vec<_> = variant_infos.iter().filter(|v| !v.is_unit).collect();
    // Cheap only when every variant payload is. Repeated payload types
    // collapse to one term (clippy's eq_op rejects duplicates).
    let mut cheap_seen = std::collections::HashSet::new();
    let cheap_terms: Vec<proc_macro2::TokenStream> = non_unit_variants
        .iter()
        .map(|v| {
            let ty = v.payload_ty.as_ref().expect("non-unit variant has a payload type");
            quote! { <#ty as pco_pack::PcoSerde>::CHEAP_TO_CLONE }
        })
        .filter(|term| cheap_seen.insert(term.to_string()))
        .collect();
    let cheap_to_clone = if cheap_terms.is_empty() {
        quote! { true }
    } else {
        let mut it = cheap_terms.into_iter();
        let first = it.next().unwrap();
        quote! { #first #(&& #it)* }
    };
    // The variant count gates blocks past the first, so blobs written by schemas with
    // fewer variants still parse. read_exact already consumed it, so only multi-block
    // readers name it.
    let multi_block = non_unit_variants.len() > 1;
    let block_count_decl: proc_macro2::TokenStream = if multi_block {
        quote! { let mut block_count = u64::from_le_bytes(len_buf) as usize; }
    } else {
        quote! {}
    };
    let variant_deserialize_blocks: proc_macro2::TokenStream = if non_unit_variants.is_empty() {
        quote! {}
    } else {
        let mut blocks = Vec::new();
        for (i, v) in non_unit_variants.iter().enumerate() {
            let field_id = syn::Ident::new(&format!("variant_{}", v.discriminant), v.ident.span());

            if i == 0 {
                let count_tick = multi_block.then(|| quote! { block_count -= 1; });
                blocks.push(quote! {
                    let #field_id = {
                        let block = pco_enum_block(src)?;
                        pco_pack::LazyReader::new(block.to_vec(), float_round, time_round)
                    };
                    #count_tick
                });
            } else {
                blocks.push(quote! {
                    let #field_id = if block_count > 0 {
                        {
                            let block = pco_enum_block(src)?;
                            pco_pack::LazyReader::new(block.to_vec(), float_round, time_round)
                        }
                    } else {
                        pco_pack::LazyReader::new(Vec::new(), float_round, time_round)
                    };
                });
            }
        }
        quote! {
            #block_count_decl
            #(#blocks)*
        }
    };

    // Variant views for enum folds: the engine narrows the mask to the fold's declared
    // variant, so only that variant's rows reach the consumer and only its block ever
    // decompresses. The view hands back the lazy block plus the payload slot.
    let variant_views: Vec<proc_macro2::TokenStream> = non_unit_variants
        .iter()
        .map(|v| {
            let payload_ty = v.payload_ty.as_ref().unwrap();
            let view_fn = format_ident!("variant_{}", v.ident);
            let disc = v.discriminant.to_string();
            let block = format_ident!("variant_{}", disc);
            let cursor = format_ident!("variant_{}_index", disc);
            let last_row = format_ident!("variant_{}_last_row", disc);
            let view_fn_lit = proc_macro2::Literal::string(&v.ident.to_string());
            let doc = format!(
                " Payload view for `Self::{}` rows: the variant's lazy block and the payload slot for the current row. ",
                v.ident
            );
            quote! {
                #[doc = #doc]
                #[allow(non_snake_case)]
                pub fn #view_fn(&mut self, row: usize) -> pco_pack::anyhow::Result<(&mut pco_pack::LazyReader<#payload_ty>, usize)> {
                    let slot = self.reader.#cursor;
                    self.reader.#cursor += 1;
                    debug_assert!(
                        slot <= row,
                        "variant_{} payload slot {} outruns row {}: every call must follow an ascending walk over this variant's rows",
                        #view_fn_lit,
                        slot,
                        row,
                    );
                    #[cfg(debug_assertions)]
                    {
                        let last = self.reader.#last_row.replace(row);
                        debug_assert!(
                            last.is_none_or(|last| row > last),
                            "variant_{} payload view called for row {} after row {}: one call per matching row, ascending",
                            #view_fn_lit,
                            row,
                            last.unwrap_or_default(),
                        );
                    }
                    Ok((&mut self.reader.#block, slot))
                }
            }
        })
        .collect();

    // Variant name to discriminant, so a fold's declared FIELDS can narrow the mask.
    let variant_disc_arms: proc_macro2::TokenStream = variant_infos
        .iter()
        .map(|v| {
            let lit = syn::LitStr::new(&v.ident.to_string(), v.ident.span());
            let disc = v.discriminant;
            quote! { #lit => Some(#disc), }
        })
        .collect();

    // Payload variants map their discriminant to their block's slot count, which the
    // slot-streaming fast path iterates in place of a mask walk.
    let slot_count_arms: proc_macro2::TokenStream = non_unit_variants
        .iter()
        .map(|v| {
            let disc = v.discriminant;
            let block = format_ident!("variant_{}", disc.to_string());
            quote! { Some(#disc) => Some(reader.#block.row_count()?), }
        })
        .collect();

    // Debug-only cross-checks that each arm flagged in POPS_PAYLOAD advanced its slot
    // cursor exactly once per delivered row: against the fast path's slot count, or
    // against the per-chunk delivered tally. The chains use `if`/`else if` equality
    // tests so a one-arm match cannot invite clippy's `single_match` in user code.
    let slot_stream_cursor_arms: proc_macro2::TokenStream = non_unit_variants
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let disc = v.discriminant;
            let cursor = format_ident!("variant_{}_index", disc.to_string());
            let name = syn::LitStr::new(&v.ident.to_string(), v.ident.span());
            let body = quote! {
                debug_assert_eq!(
                    reader.#cursor,
                    slot_count,
                    "fold arm for variant {} advanced its payload slot cursor {} times for {} streamed slots",
                    #name, reader.#cursor, slot_count,
                );
            };
            if i == 0 {
                quote! { if declared[0] == #disc { #body } }
            } else {
                quote! { else if declared[0] == #disc { #body } }
            }
        })
        .collect();

    let delivered_cursor_arms: proc_macro2::TokenStream = non_unit_variants
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let disc = v.discriminant;
            let cursor = format_ident!("variant_{}_index", disc.to_string());
            let name = syn::LitStr::new(&v.ident.to_string(), v.ident.span());
            let body = quote! {
                debug_assert_eq!(
                    reader.#cursor,
                    __pco_delivered[__pco_arm],
                    "fold arm {} for variant {} advanced its payload slot cursor {} times for {} delivered rows",
                    __pco_arm, #name, reader.#cursor, __pco_delivered[__pco_arm],
                );
            };
            if i == 0 {
                quote! { if declared[__pco_arm] == #disc { #body } }
            } else {
                quote! { else if declared[__pco_arm] == #disc { #body } }
            }
        })
        .collect();

    let float_round_attr = attrs.float_round.unwrap_or(0);
    let time_round_attr =
        attrs.time_round.as_ref().map(|expr| quote! { #expr }).unwrap_or(quote! { Default::default() });
    // Without the attribute the trait's default chunk size applies, so enum writes
    // flush full chunks exactly like struct writes.
    let chunk_size_decl = attrs.chunk_size.map(|v| quote! { const CHUNK_SIZE: usize = #v; });
    quote! {
        const _: () = {
            #[allow(unused)]
            use pco_pack::anyhow::Context;
            use pco_pack::serde::Serialize;

            #[doc = concat!("Writer container for accumulating ", stringify!(#name), " rows.")]
            #[derive(Default)]
            pub struct Writer {
                row_variant: pco_pack::NumberWriter<i64>,
                #struct_field_decls
            }

            #[doc = concat!("Reader state for ", stringify!(#name), " data.")]
            #[derive(Clone, Default)]
            pub struct Reader {
                row_variant: pco_pack::LazyReader<i64>,
                #reader_field_decls
                #reader_variant_index_decls
            }

            /// Column view for enum consumers. `hydrate` reconstructs variants via
            /// `PcoSerde::get`; a `variant_*` view exposes one variant's payload block
            /// for pattern-`fold!` projections that skip the payload struct entirely.
            pub struct Columns<'a> {
                reader: &'a mut Reader,
            }

            impl Columns<'_> {
                #(#variant_views)*
            }

            impl pco_pack::PcoSerde for #name {
                type Writer = Writer;
                type Reader = Reader;

                const CHEAP_TO_CLONE: bool = #cheap_to_clone;

                fn write(data: Vec<Self>, float_round: u32, time_round: pco_pack::chrono::Duration) -> pco_pack::anyhow::Result<Vec<u8>> {
                    let mut writer = Writer::default();
                    for item in data {
                        match item {
                            #serialize_arms
                        }
                    }
                    let mut out = Vec::new();
                    // Framed like the payload blocks so the reader slices the
                    // discriminant without decoding it; slot-streaming folds never
                    // decode it at all.
                    let disc_block = <i64 as pco_pack::PcoSerde>::write(writer.row_variant.values, float_round, time_round)?;
                    out.extend_from_slice(&(disc_block.len() as u64).to_le_bytes());
                    out.extend_from_slice(&disc_block);
                    let non_unit_count: usize = #non_unit_count;
                    out.extend_from_slice(&(non_unit_count as u64).to_le_bytes());
                    #variant_serialize_blocks
                    Ok(out)
                }

                fn read(
                    src: &mut std::io::Cursor<&[u8]>, float_round: u32, time_round: pco_pack::chrono::Duration,
                ) -> pco_pack::anyhow::Result<Self::Reader> {
                    let row_variant = {
                        let block = pco_enum_block(src)?;
                        pco_pack::LazyReader::new(block.to_vec(), float_round, time_round)
                    };
                    let mut len_buf = [0u8; 8];
                    <std::io::Cursor<&[u8]> as std::io::Read>::read_exact(src, &mut len_buf)?;
                    #variant_deserialize_blocks
                    Ok(Reader { row_variant, #field_names_only })
                }

                fn validate_bounds(reader: &mut Self::Reader) -> pco_pack::anyhow::Result<Option<usize>> {
                    Ok(Some(reader.row_variant.row_count()?))
                }

                fn get(reader: &mut Self::Reader, index: usize) -> pco_pack::anyhow::Result<Option<Self>> {
                    let discriminant = <i64 as pco_pack::PcoSerde>::get(reader.row_variant.reader_mut()?, index)?;
                    match discriminant {
                        #get_arms
                        None => Ok(None),
                        Some(other) => Err(pco_pack::anyhow::anyhow!(
                            "Enum '{}' has no variant with discriminant {}",
                            stringify!(#name),
                            other
                        )),
                    }
                }
            }

            /// Slice the next length-prefixed block out of the blob, returning an error
            /// for truncated or format-mismatched blobs.
            fn pco_enum_block<'a>(
                src: &mut std::io::Cursor<&'a [u8]>,
            ) -> pco_pack::anyhow::Result<&'a [u8]> {
                let mut len_buf = [0u8; 8];
                <std::io::Cursor<&[u8]> as std::io::Read>::read_exact(src, &mut len_buf)?;
                let block_len = u64::from_le_bytes(len_buf) as usize;
                let start = <std::io::Cursor<&[u8]>>::position(src) as usize;
                let end = start.checked_add(block_len).ok_or_else(|| {
                    pco_pack::anyhow::anyhow!("enum block length {} overflows the buffer", block_len)
                })?;
                let buf = *src.get_ref();
                let block = <[u8]>::get(buf, start..end).ok_or_else(|| {
                    pco_pack::anyhow::anyhow!(
                        "enum block of {} bytes at offset {} runs past the {}-byte chunk",
                        block_len,
                        start,
                        <[u8]>::len(buf)
                    )
                })?;
                <std::io::Cursor<&[u8]>>::set_position(src, end as u64);
                Ok(block)
            }

            impl pco_pack::PcoFilter for #name {
                fn filter_bulk(
                    reader: &mut Self::Reader,
                    _field: usize,
                    filter: &pco_pack::Filter,
                    matches: &mut pco_pack::FilterMask,
                ) -> pco_pack::anyhow::Result<()> {
                    reader.row_variant.filter_in_group(filter, matches)
                }

                fn filter_match(value: &Self, filter: &pco_pack::Filter) -> bool {
                    match value {
                        #matches_arms
                    }
                }

                fn filter_nested(
                    _reader: &mut Self::Reader,
                    _path: &[usize],
                    _filter: &pco_pack::Filter,
                    _matches: &mut pco_pack::FilterMask,
                ) -> pco_pack::anyhow::Result<()> {
                    unreachable!("filter_nested not supported for '{}'", stringify!(#name));
                }

                fn resolve_filter(
                    path: &str,
                    json: &pco_pack::serde_json::Value,
                ) -> pco_pack::anyhow::Result<pco_pack::ResolvedFilter> {
                    if path.is_empty() {
                        // Handle range syntax: {"start": min, "end": max}
                        // (get() on a non-object yields None, so this matches objects only)
                        if let Some((start_val, end_val)) = json.get("start").zip(json.get("end")) {
                            let start = start_val.as_i64()
                                .with_context(|| format!("Range start must be an integer, got {}", pco_pack::json_type_name(start_val)))?;
                            let end = end_val.as_i64()
                                .with_context(|| format!("Range end must be an integer, got {}", pco_pack::json_type_name(end_val)))?;
                            return Ok(pco_pack::ResolvedFilter {
                                path: vec![0],
                                filter: pco_pack::Filter::Range(start..=end),
                            });
                        }
                        // Handle inclusion syntax: [1, 2, 3]
                        if let Some(arr) = json.as_array().filter(|a| !a.is_empty()) {
                            let ints: Vec<i64> = arr.iter()
                                .filter_map(|v| v.as_i64())
                                .collect();
                            if ints.len() == arr.len() {
                                return Ok(pco_pack::ResolvedFilter {
                                    path: vec![0],
                                    filter: pco_pack::Filter::InclusionI64(ints.into_iter().collect()),
                                });
                            }
                        }
                        let discriminant: Option<i64> = json.as_i64().or_else(|| json.as_f64().map(|f| f as i64));
                        let discriminant = discriminant
                            .with_context(|| format!("Expected i64 discriminant value for enum filter, got {}", pco_pack::json_type_name(json)))?;
                        return Ok(pco_pack::ResolvedFilter {
                            path: vec![0],
                            filter: pco_pack::Filter::I64(discriminant),
                        });
                    }
                    // A query key may name a variant directly: `{"Tick": true}` keeps only
                    // that variant's rows. Payload paths never resolve; enum filtering is
                    // variant-level, which is what keeps pattern folds' occurrence cursors
                    // aligned (a mask can only drop whole variants).
                    if path.contains('.') {
                        return Err(pco_pack::anyhow::anyhow!(
                            "Enum '{}' has no nested fields; cannot resolve path '{}'",
                            stringify!(#name),
                            path
                        ));
                    }
                    let variant_disc: Option<i64> = match path {
                        #variant_disc_arms
                        _ => None,
                    };
                    let Some(discriminant) = variant_disc else {
                        return Err(pco_pack::anyhow::anyhow!(
                            "Enum '{}' has no variant '{}'; query keys are variant names or the empty discriminant key",
                            stringify!(#name),
                            path
                        ));
                    };
                    match json.as_bool() {
                        Some(true) => Ok(pco_pack::ResolvedFilter {
                            path: vec![0],
                            filter: pco_pack::Filter::I64(discriminant),
                        }),
                        Some(false) => Err(pco_pack::anyhow::anyhow!(
                            "Enum '{}' variant '{}' filter must be true; exclude a variant by listing the discriminants to keep under the empty key",
                            stringify!(#name),
                            path
                        )),
                        None => Err(pco_pack::anyhow::anyhow!(
                            "Enum '{}' variant '{}' filter must be a boolean, got {}",
                            stringify!(#name),
                            path,
                            pco_pack::json_type_name(json)
                        )),
                    }
                }
            }

            impl pco_pack::PcoPack for #name {
                #chunk_size_decl

                type Reader = <Self as pco_pack::PcoSerde>::Reader;
                type Chunk = pco_pack::serde_bytes::ByteBuf;
                type Filter = serde_json::Value;
                type Columns<'a> = Columns<'a>;

                fn hydrate(columns: &mut Self::Columns<'_>, row: usize) -> pco_pack::anyhow::Result<Self> {
                    <Self as pco_pack::PcoSerde>::get(columns.reader, row)?
                        .ok_or_else(|| pco_pack::anyhow::anyhow!("Missing enum variant at index {}", row))
                }

                fn columns(reader: &mut <Self as pco_pack::PcoSerde>::Reader) -> Self::Columns<'_> {
                    Columns { reader }
                }

                fn fold_value<C>(
                    chunks: &[Self::Chunk],
                    query: &pco_pack::serde_json::Value,
                    fields: &[&str],
                    consumer: &mut C,
                ) -> pco_pack::anyhow::Result<()>
                where
                    C: pco_pack::Consumer<Self>,
                {
                    if !fields.is_empty() {
                        return Err(pco_pack::anyhow::anyhow!(
                            "Enum '{}' does not support field matches",
                            stringify!(#name)
                        ));
                    }
                    // A pattern fold declares its variants through FIELDS (one or more
                    // variant names); hydrating consumers declare nothing. The declared
                    // set narrows the mask.
                    let mut declared: Vec<i64> = Vec::new();
                    for &name in <C as pco_pack::Consumer<Self>>::FIELDS {
                        let disc: Option<i64> = match name {
                            #variant_disc_arms
                            _ => None,
                        };
                        let Some(disc) = disc else {
                            return Err(pco_pack::anyhow::anyhow!(
                                "Enum '{}' has no variant '{}'",
                                stringify!(#name),
                                name
                            ));
                        };
                        declared.push(disc);
                    }
                    let execution_plan = <Self as pco_pack::PcoFilter>::resolve_query(query)?;
                    // With nothing to filter, every row streams; skip the mask entirely.
                    let stream_all = execution_plan.is_empty() && declared.is_empty();
                    // Every enum filter tests the discriminant column, so equality and
                    // inclusion steps together bound exactly which variants survive; a
                    // range step bounds without enumerating and clears the pin set.
                    // Serves the fast path and the narrowing check below.
                    let mut pinned: Option<Vec<i64>> = None;
                    let mut bounded = true;
                    for step in &execution_plan {
                        if !bounded {
                            break;
                        }
                        match &step.filter {
                            pco_pack::Filter::I64(target) if step.path.first() == Some(&0) => {
                                pinned = Some(match pinned {
                                    None => vec![*target],
                                    Some(cur) => cur.into_iter().filter(|d| d == target).collect(),
                                });
                            }
                            pco_pack::Filter::InclusionI64(targets) if step.path.first() == Some(&0) => {
                                pinned = Some(match pinned {
                                    None => targets.iter().copied().collect(),
                                    Some(cur) => cur.into_iter().filter(|d| targets.contains(d)).collect(),
                                });
                            }
                            _ => bounded = false,
                        }
                    }
                    // Pattern and binding folds over a single payload variant stream
                    // slots directly - no discriminant decode, row-count validation, or
                    // mask - whenever no query step can drop a declared row: no steps at
                    // all, or every step pins exactly that variant. Membership folds
                    // read no slots and keep the mask path, which counts rows through
                    // the discriminant without decoding the payload. Unit variants have
                    // no slots, and a multi-arm fold needs the discriminant to dispatch.
                    let pops = <C as pco_pack::Consumer<Self>>::POPS_PAYLOAD;
                    let slot_stream = bounded
                        && declared.len() == 1
                        && pops.len() == 1
                        && pops[0]
                        && match &pinned {
                            Some(pins) => pins.as_slice() == declared.as_slice(),
                            None => execution_plan.is_empty(),
                        };
                    // Scratch mask reused across chunks; sized per chunk in the mask path.
                    let mut matches = pco_pack::FilterMask::ones(0);
                    'chunks: for chunk in chunks {
                        if chunk.is_empty() {
                            continue 'chunks;
                        }
                        let mut cursor = std::io::Cursor::new(chunk.as_slice());
                        let mut reader = <Self as pco_pack::PcoSerde>::read(&mut cursor, #float_round_attr, #time_round_attr)?;

                        if slot_stream {
                            let slot_count: Option<usize> = match Some(declared[0]) {
                                #slot_count_arms
                                _ => None,
                            };
                            if let Some(slot_count) = slot_count {
                                let mut columns = Columns { reader: &mut reader };
                                for slot in 0..slot_count {
                                    consumer.consume(&mut columns, slot)?;
                                }
                                // `slot_stream` already implies `pops[0]`, so the
                                // chain needs no extra guard; it matches the single
                                // declared variant's discriminant and checks its
                                // cursor against the streamed slot count.
                                #[cfg(debug_assertions)]
                                {
                                    #slot_stream_cursor_arms;
                                }
                                continue 'chunks;
                            }
                        }

                        // Debug-only tally of rows delivered to each declared arm, so
                        // the cursor cross-check below can tell a fold that skipped a
                        // row from one that double-consumed.
                        #[cfg(debug_assertions)]
                        let mut __pco_delivered = vec![0usize; declared.len()];

                        let row_count: usize = <Self as pco_pack::PcoSerde>::validate_bounds(&mut reader)
                            .context("Invalid enum payload")?
                            .ok_or_else(|| pco_pack::anyhow::anyhow!("No rows in enum data"))?;

                        if stream_all {
                            let mut columns = Columns { reader: &mut reader };
                            for row_index in 0..row_count {
                                consumer.consume(&mut columns, row_index)?;
                            }
                            continue 'chunks;
                        }

                        matches.reset_ones(row_count);
                        for step in &execution_plan {
                            <Self as pco_pack::PcoFilter>::filter_step(&mut reader, &step.path, &step.filter, &mut matches)?;
                            // This chunk contributed no survivors; try the next one.
                            if !matches.any_set_in_range(0..row_count) {
                                continue 'chunks;
                            }
                        }

                        if !declared.is_empty() {
                            // The narrowing keeps declared variants' rows. It is
                            // redundant when the query steps already pin the
                            // discriminant inside the declared set; the walk then
                            // touches the column once instead of twice.
                            let redundant = pinned
                                .as_ref()
                                .is_some_and(|pins| bounded && pins.iter().all(|d| declared.contains(d)));
                            if !redundant {
                                // Narrow the whole declared set in one pass over the column.
                                let disc_filter = if declared.len() == 1 {
                                    pco_pack::Filter::I64(declared[0])
                                } else {
                                    pco_pack::Filter::InclusionI64(declared.iter().copied().collect())
                                };
                                <Self as pco_pack::PcoFilter>::filter_bulk(&mut reader, 0, &disc_filter, &mut matches)?;
                                if !matches.any_set_in_range(0..row_count) {
                                    continue 'chunks;
                                }
                            }
                        }
                        if declared.len() > 1 {
                            // Multi-arm fold: every declared variant is already pinned
                            // into the mask, so each row dispatches to the arm at its
                            // discriminant's position. The column decodes once and
                            // lookups are random access, one read per surviving row.
                            matches.for_each_set_row(row_count, |row_index| {
                                let disc = <i64 as pco_pack::PcoSerde>::get(reader.row_variant.reader_mut()?, row_index)?
                                    .ok_or_else(|| pco_pack::anyhow::anyhow!("Missing discriminant at row {}", row_index))?;
                                let Some(variant) = declared.iter().position(|&d| d == disc) else {
                                    return Err(pco_pack::anyhow::anyhow!(
                                        "Row {} carries discriminant {} outside the declared variants",
                                        row_index,
                                        disc
                                    ));
                                };
                                #[cfg(debug_assertions)]
                                {
                                    __pco_delivered[variant] += 1;
                                }
                                let mut columns = Columns { reader: &mut reader };
                                consumer.consume_variant(&mut columns, row_index, variant)
                            })?;
                        } else {
                            let mut columns = Columns { reader: &mut reader };
                            matches.for_each_set_row(row_count, |row_index| {
                                #[cfg(debug_assertions)]
                                if let Some(n) = __pco_delivered.first_mut() {
                                    *n += 1;
                                }
                                consumer.consume(&mut columns, row_index)
                            })?;
                        }
                        // A payload arm that skipped or repeated its view call misaligns
                        // every later slot; this is the last place the engine can still
                        // see the discrepancy.
                        #[cfg(debug_assertions)]
                        if !pops.is_empty() {
                            debug_assert!(
                                pops.len() == declared.len(),
                                "Consumer::POPS_PAYLOAD must align with FIELDS"
                            );
                            for __pco_arm in 0..pops.len().min(declared.len()) {
                                if !pops[__pco_arm] {
                                    continue;
                                }
                                #delivered_cursor_arms;
                            }
                        }
                    }
                    Ok(())
                }

                fn write(data: Vec<Self>) -> pco_pack::anyhow::Result<Vec<Self::Chunk>> {
                    // Chunk like structs: one self-contained block per CHUNK_SIZE rows;
                    // empty input writes no chunks.
                    let mut result = Vec::new();
                    let mut rows = data.into_iter();
                    while let Some(first) = rows.next() {
                        let mut piece = Vec::with_capacity(Self::CHUNK_SIZE);
                        piece.push(first);
                        piece.extend(rows.by_ref().take(Self::CHUNK_SIZE - 1));
                        let chunk = <Self as pco_pack::PcoSerde>::write(piece, #float_round_attr, #time_round_attr)?;
                        result.push(chunk.into());
                    }
                    Ok(result)
                }

                fn read(chunks: Vec<Self::Chunk>) -> pco_pack::anyhow::Result<Vec<Self>> {
                    let mut result = Vec::new();
                    for chunk in chunks {
                        let mut cursor = std::io::Cursor::new(chunk.as_slice());
                        let mut reader = <Self as pco_pack::PcoSerde>::read(&mut cursor, #float_round_attr, #time_round_attr)?;
                        let row_count = <Self as pco_pack::PcoSerde>::validate_bounds(&mut reader)
                            .context("Invalid enum payload")?
                            .ok_or_else(|| pco_pack::anyhow::anyhow!("No rows in enum data"))?;
                        for row_idx in 0..row_count {
                            // `get`'s own errors (framing, unknown discriminant) are
                            // descriptive; a blanket context here would mislabel them
                            // all as truncation.
                            result.push(<Self as pco_pack::PcoSerde>::get(&mut reader, row_idx)?
                                .ok_or_else(|| pco_pack::anyhow::anyhow!("Missing enum variant at index {}", row_idx))?);
                        }
                    }
                    Ok(result)
                }

                fn filter_value(
                    chunks: &[Self::Chunk],
                    query: &pco_pack::serde_json::Value,
                    fields: &[&str],
                ) -> pco_pack::anyhow::Result<Vec<Self>> {
                    if !fields.is_empty() {
                        return Err(pco_pack::anyhow::anyhow!(
                            "Enum '{}' does not support field matches",
                            stringify!(#name)
                        ));
                    }
                    let execution_plan = <Self as pco_pack::PcoFilter>::resolve_query(query)?;
                    let mut result = Vec::new();
                    'chunks: for chunk in chunks {
                        if chunk.is_empty() {
                            continue 'chunks;
                        }
                        let mut cursor = std::io::Cursor::new(chunk.as_slice());
                        let mut reader = <Self as pco_pack::PcoSerde>::read(&mut cursor, #float_round_attr, #time_round_attr)?;
                        let row_count: usize = <Self as pco_pack::PcoSerde>::validate_bounds(&mut reader)
                            .context("Invalid enum payload")?
                            .ok_or_else(|| pco_pack::anyhow::anyhow!("No rows in enum data"))?;

                        let mut matches = pco_pack::FilterMask::ones(row_count);
                        for step in &execution_plan {
                            <Self as pco_pack::PcoFilter>::filter_step(
                                &mut reader,
                                &step.path,
                                &step.filter,
                                &mut matches,
                            )?;
                            // This chunk contributed no survivors; try the next one.
                            if !matches.any_set_in_range(0..row_count) {
                                continue 'chunks;
                            }
                        }

                        let mut chunk_hits = Vec::with_capacity(matches.count_ones());
                        matches.for_each_set_row(row_count, |actual_row_index| {
                            chunk_hits.push(<Self as pco_pack::PcoSerde>::get(&mut reader, actual_row_index)?
                                .ok_or_else(|| pco_pack::anyhow::anyhow!("Missing enum variant at index {}", actual_row_index))?);
                            Ok(())
                        })?;
                        result.extend(chunk_hits);
                    }
                    Ok(result)
                }

                fn to_bytes(chunks: &[Self::Chunk]) -> pco_pack::anyhow::Result<Vec<u8>> {
                    let mut out = Vec::new();
                    chunks.serialize(&mut pco_pack::rmp_serde::Serializer::new(&mut out).with_struct_map())?;
                    Ok(out)
                }

                fn from_bytes(bytes: &[u8]) -> pco_pack::anyhow::Result<Vec<Self::Chunk>> {
                    Ok(pco_pack::rmp_serde::from_slice(bytes)?)
                }

                fn resolve_fields(_query: &pco_pack::serde_json::Value, _fields: &[&str]) -> pco_pack::anyhow::Result<Vec<&'static str>> {
                    Ok(Vec::new())
                }
            }

            // Existence markers read by `fold!` expansions: naming a variant in a fold
            // pattern references the matching const, so a misspelled variant is a
            // compile error instead of a runtime "no variant" failure. They exist only
            // because the derive ran, doubling as the "derives PcoPack" check.
            #[allow(non_upper_case_globals)]
            impl #name {
                #(
                    #[doc(hidden)]
                    pub const #variant_probe_idents: () = ();
                )*
            }
        };
    }
}
