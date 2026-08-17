use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
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
                    .to_compile_error()
                    .into();
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
                .to_compile_error()
                .into();
            }
        }
    }

    let payload_field_ids: Vec<syn::Ident> = variant_infos
        .iter()
        .filter(|v| !v.is_unit)
        .map(|v| syn::Ident::new(&format!("variant_{}", v.discriminant), v.ident.span()))
        .collect();

    let default_field_values: proc_macro2::TokenStream = payload_field_ids
        .iter()
        .map(|field_name| {
            quote! { #field_name: Vec::new(), }
        })
        .collect();

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
            quote! { #index_name: usize, }
        })
        .collect();

    let field_names_only: proc_macro2::TokenStream = payload_field_ids
        .iter()
        .map(|field_id| {
            let index_name = syn::Ident::new(&format!("{}_index", field_id), field_id.span());
            quote! {
                #field_id,
                #index_name: 0,
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
                        writer.row_variant.values.push(#discriminant as i64);
                    }
                }
            } else {
                let payload_field = syn::Ident::new(&format!("variant_{}", discriminant), v.ident.span());
                _ = v.payload_ty.as_ref();
                quote! {
                    Self::#variant_ident(inner) => {
                        writer.row_variant.values.push(#discriminant as i64);
                        writer.#payload_field.push(inner);
                    }
                }
            }
        })
        .collect();

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
                        return Ok(Some(Self::#variant_ident(reader.#payload_field.pop_inner(idx)?)));
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
    let variant_deserialize_blocks: proc_macro2::TokenStream = if non_unit_variants.is_empty() {
        quote! {}
    } else {
        // Schema evolution: first variant block always present; later blocks conditional
        // to support older schemas with fewer variants.
        let mut blocks = Vec::new();
        for (i, v) in non_unit_variants.iter().enumerate() {
            let field_id = syn::Ident::new(&format!("variant_{}", v.discriminant), v.ident.span());

            if i == 0 {
                blocks.push(quote! {
                    let #field_id = {
                        let mut block_buf = [0u8; 8];
                        <std::io::Cursor<&[u8]> as std::io::Read>::read_exact(src, &mut block_buf)?;
                        let block_len = u64::from_le_bytes(block_buf) as usize;
                        if block_len > 0 {
                            let current_pos = src.position() as usize;
                            let compressed = src.get_ref()[current_pos..current_pos + block_len].to_vec();
                            src.set_position((current_pos + block_len) as u64);
                            pco_pack::LazyReader::new(compressed, _float_round, _time_round.clone())
                        } else {
                            pco_pack::LazyReader::new(Vec::new(), _float_round, _time_round.clone())
                        }
                    };
                    _block_count -= 1;
                });
            } else {
                blocks.push(quote! {
                    let #field_id = if _block_count > 0 {
                        {
                            let mut block_buf = [0u8; 8];
                            <std::io::Cursor<&[u8]> as std::io::Read>::read_exact(src, &mut block_buf)?;
                            let block_len = u64::from_le_bytes(block_buf) as usize;
                            if block_len > 0 {
                                let current_pos = src.position() as usize;
                                let compressed = src.get_ref()[current_pos..current_pos + block_len].to_vec();
                                src.set_position((current_pos + block_len) as u64);
                                pco_pack::LazyReader::new(compressed, _float_round, _time_round.clone())
                            } else {
                                pco_pack::LazyReader::new(Vec::new(), _float_round, _time_round.clone())
                            }
                        }
                    } else {
                        pco_pack::LazyReader::new(Vec::new(), _float_round, _time_round.clone())
                    };
                });
            }
        }
        quote! {
            let mut _block_count = _num_variants;
            #(#blocks)*
        }
    };

    let float_round_attr = attrs.float_round.unwrap_or(0);
    let time_round_attr =
        attrs.time_round.as_ref().map(|expr| quote! { #expr }).unwrap_or(quote! { Default::default() });
    quote! {
        const _: () = {
            #[allow(unused)]
            use pco_pack::anyhow::Context;
            use pco_pack::serde::Serialize;

            #[doc = concat!("Writer container for accumulating ", stringify!(#name), " rows.")]
            pub struct Writer {
                row_variant: pco_pack::NumberWriter<i64>,
                #struct_field_decls
            }

            impl Default for Writer {
                fn default() -> Self {
                    Self {
                        row_variant: pco_pack::NumberWriter::default(),
                        #default_field_values
                    }
                }
            }

            #[doc = concat!("Reader state for ", stringify!(#name), " data.")]
            #[derive(Clone, Default)]
            pub struct Reader {
                row_variant: pco_pack::NumberReader<i64>,
                #reader_field_decls
                #reader_variant_index_decls
            }

            impl pco_pack::PcoSerde for #name {
                type Writer = Writer;
                type Reader = Reader;

                fn write(data: Vec<Self>, float_round: u32, time_round: pco_pack::chrono::Duration) -> pco_pack::anyhow::Result<Vec<u8>> {
                    let mut writer = Writer::default();
                    for item in data {
                        match item {
                            #serialize_arms
                        }
                    }
                    let mut out = Vec::new();
                    out.extend_from_slice(&<i64 as pco_pack::PcoSerde>::write(writer.row_variant.values, float_round, time_round)?);
                    let non_unit_count: usize = #non_unit_count;
                    out.extend_from_slice(&(non_unit_count as u64).to_le_bytes());
                    #variant_serialize_blocks
                    Ok(out)
                }

                fn read(
                    src: &mut std::io::Cursor<&[u8]>, _float_round: u32, _time_round: pco_pack::chrono::Duration,
                ) -> pco_pack::anyhow::Result<Self::Reader> {
                    let row_variant = <i64 as pco_pack::PcoSerde>::read(src, 0, Default::default())?;
                    let mut len_buf = [0u8; 8];
                    <std::io::Cursor<&[u8]> as std::io::Read>::read_exact(src, &mut len_buf)?;
                    let _num_variants = u64::from_le_bytes(len_buf) as usize;
                    #variant_deserialize_blocks
                    Ok(Reader { row_variant, #field_names_only })
                }

                fn validate_bounds(reader: &mut Self::Reader) -> pco_pack::anyhow::Result<Option<usize>> {
                    Ok(Some(reader.row_variant.values.len()))
                }

                fn get(reader: &mut Self::Reader, index: usize) -> pco_pack::anyhow::Result<Option<Self>> {
                    let discriminant = <i64 as pco_pack::PcoSerde>::get(&mut reader.row_variant, index)?;
                    match discriminant {
                        #get_arms
                        None | Some(_) => Ok(Some(Self::default())),
                    }
                }
            }

            impl pco_pack::PcoFilter for #name {
                fn filter_bulk(
                    reader: &mut Self::Reader,
                    _field: usize,
                    filter: &pco_pack::Filter,
                    matches: &mut pco_pack::FilterMask,
                ) -> pco_pack::anyhow::Result<()> {
                    <i64 as pco_pack::PcoFilter>::filter_bulk(
                        &mut reader.row_variant, 0, filter, matches,
                    )?;
                    Ok(())
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
                        if let pco_pack::serde_json::Value::Object(obj) = json {
                            if let (Some(start_val), Some(end_val)) = (obj.get("start"), obj.get("end")) {
                                let start = start_val.as_i64().context("Range start must be an integer")?;
                                let end = end_val.as_i64().context("Range end must be an integer")?;
                                return Ok(pco_pack::ResolvedFilter {
                                    path: vec![0],
                                    filter: pco_pack::Filter::Range(start..=end),
                                });
                            }
                        }
                        // Handle inclusion syntax: [1, 2, 3]
                        if let pco_pack::serde_json::Value::Array(arr) = json {
                            if !arr.is_empty() {
                                let ints: Vec<i64> = arr.iter()
                                    .filter_map(|v| v.as_i64())
                                    .collect();
                                if ints.len() == arr.len() {
                                    return Ok(pco_pack::ResolvedFilter {
                                        path: vec![0],
                                        filter: pco_pack::Filter::InclusionI64(ints),
                                    });
                                }
                            }
                        }
                        let discriminant: Option<i64> = json.as_i64().or_else(|| json.as_f64().map(|f| f as i64));
                        let discriminant = discriminant.context("Expected i64 discriminant value for enum filter")?;
                        return Ok(pco_pack::ResolvedFilter {
                            path: vec![0],
                            filter: pco_pack::Filter::I64(discriminant),
                        });
                    }
                    Err(pco_pack::anyhow::anyhow!(
                        "Enum '{}' has no nested fields; cannot resolve path '{}'",
                        stringify!(#name),
                        path
                    ))
                }
            }

            impl pco_pack::PcoPack for #name {
                type Reader = <Self as pco_pack::PcoSerde>::Reader;
                type Chunk = ::std::vec::Vec<u8>;
                type Filter = serde_json::Value;

                fn write(data: Vec<Self>) -> pco_pack::anyhow::Result<Vec<Self::Chunk>> {
                    Ok(vec![<Self as pco_pack::PcoSerde>::write(data, #float_round_attr, #time_round_attr)?])
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
                            result.push(<Self as pco_pack::PcoSerde>::get(&mut reader, row_idx)
                                .context("Unexpected end of data")?
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
                    let all_bytes: Vec<u8> = chunks
                        .iter()
                        .flat_map(|b| b.iter().copied())
                        .collect();
                    if all_bytes.is_empty() {
                        return Ok(Vec::new());
                    }
                    let mut cursor = std::io::Cursor::new(all_bytes.as_slice());
                    let mut reader = <Self as pco_pack::PcoSerde>::read(&mut cursor, #float_round_attr, #time_round_attr)?;
                    let row_count: usize = <Self as pco_pack::PcoSerde>::validate_bounds(&mut reader)
                        .context("Invalid enum payload")?
                        .ok_or_else(|| pco_pack::anyhow::anyhow!("No rows in enum data"))?;

                    let execution_plan = <Self as pco_pack::PcoFilter>::resolve_query(query)?;
                    let mut matches = pco_pack::FilterMask::ones(row_count);
                    for step in &execution_plan {
                        <Self as pco_pack::PcoFilter>::filter_step(
                            &mut reader,
                            &step.path,
                            &step.filter,
                            &mut matches,
                        )?;
                        // Early exit if no rows match anymore.
                        if !matches.any_set_in_range(0..row_count) {
                            return Ok(Vec::new());
                        }
                    }

                    let mut result = Vec::with_capacity(matches.count_ones());
                    {
                        let raw_chunks = matches.as_raw_slice();
                        let mut base_row_index: usize = 0;
                        let total_chunks = (row_count + 63) / 64;
                        for (chunk_idx, &chunk) in raw_chunks.iter().enumerate() {
                            if chunk_idx >= total_chunks {
                                break;
                            }
                            let remaining_bits = row_count.saturating_sub(base_row_index);
                            let masked_chunk = if remaining_bits < 64 {
                                chunk & ((1u64 << remaining_bits) - 1)
                            } else {
                                chunk
                            };
                            let mut current_chunk = masked_chunk;
                            while current_chunk != 0 {
                                let skip = current_chunk.trailing_zeros() as usize;
                                let actual_row_index = base_row_index + skip;
                                result.push(<Self as pco_pack::PcoSerde>::get(&mut reader, actual_row_index)
                                    .context("Unexpected end of data")?
                                    .ok_or_else(|| pco_pack::anyhow::anyhow!("Missing enum variant at index {}", actual_row_index))?);
                                current_chunk &= current_chunk - 1;
                            }
                            base_row_index += 64;
                        }
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
        };
    }
}
