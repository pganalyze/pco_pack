use super::*;
use quote::quote;

/// Generate the PcoSerde and PcoFilter impls for the struct.
pub fn generate(sg: &StructGen) -> proc_macro2::TokenStream {
    let name = &sg.name;
    let group_key_type = &sg.group_key_type;
    let group_key_tuple_ref = sg.group_key_tuple_ref();
    let col_vectors_init = sg.col_vectors_init();
    let col_vectors_push = sg.col_vectors_push();
    let timestamp_sort_indices = sg.timestamp_sort_indices();
    let timeline_merge_from_indices = sg.timeline_merge_from_indices();
    let timestamp_start_end_from_indices_var = sg.timestamp_start_end_from_indices_var("indices");
    let timestamp_start_end = sg.timestamp_start_end();
    let compress_payload = sg.compress_payload();
    let wrapper_assigns = sg.wrapper_assigns();
    let reader_fields = sg.reader_fields();
    let eval_filters = sg.eval_filters();
    let nested_eval_filters = sg.nested_eval_filters();
    let schema_arms = sg.schema_arms();
    let first_column_arms = sg.first_column_arms();

    let has_timeline_ts = sg.timestamp_field().map(|f| super::type_helpers::is_timeline_type(&f.ty)).unwrap_or(false);

    let has_ts = sg.has_timestamp;
    let has_index = !sg.index_fields().is_empty();

    let group_enum_loop = if has_index {
        quote! {
            for (i, rec) in data.iter().enumerate() {
                let key = #group_key_tuple_ref;
                groups.entry(key).or_default().push(i);
            }
        }
    } else {
        quote! {
            for (i, _) in data.iter().enumerate() {
                groups.entry(()).or_default().push(i);
            }
        }
    };

    let pco_serde_write_body = if has_timeline_ts {
        let key_pat = if has_index {
            quote! { key }
        } else {
            quote! { _key }
        };
        quote! {
            let mut records: Vec<Chunk> = Vec::with_capacity(groups.len());
            for (#key_pat, mut indices) in groups {
                #timestamp_sort_indices
                #timeline_merge_from_indices
                #(#col_vectors_init)*
                for rec in &group_rows {
                    #(#col_vectors_push)*
                }
                #(#compress_payload)*
                #timestamp_start_end
                records.push(Chunk { #(#wrapper_assigns)* });
            }
        }
    } else if has_ts {
        let key_pat = if has_index {
            quote! { key }
        } else {
            quote! { _key }
        };
        quote! {
            let mut records: Vec<Chunk> = Vec::with_capacity(groups.len());
            for (#key_pat, mut indices) in groups {
                #timestamp_sort_indices
                #(#col_vectors_init)*
                for &idx in indices.iter() {
                    let rec = &data[idx];
                    #(#col_vectors_push)*
                }
                #(#compress_payload)*
                #timestamp_start_end_from_indices_var
                records.push(Chunk { #(#wrapper_assigns)* });
            }
        }
    } else {
        let key_pat = if has_index {
            quote! { key }
        } else {
            quote! { _key }
        };
        quote! {
            let mut records: Vec<Chunk> = Vec::with_capacity(groups.len());
            for (#key_pat, indices) in groups {
                #(#col_vectors_init)*
                for &idx in indices.iter() {
                    let rec = &data[idx];
                    #(#col_vectors_push)*
                }
                #(#compress_payload)*
                records.push(Chunk { #(#wrapper_assigns)* });
            }
        }
    };

    let pco_serde = quote! {
        impl pco_pack::PcoSerde for #name {
            type Writer = Writer;
            type Reader = Reader;
            fn write(data: Vec<Self>, _float_round: u32, _time_round: pco_pack::chrono::Duration) -> pco_pack::anyhow::Result<Vec<u8>> {
                let mut groups: pco_pack::ahash::HashMap<#group_key_type, Vec<usize>> = Default::default();
                #group_enum_loop
                #pco_serde_write_body
                let mut out = Vec::new();
                for rec in records {
                    let mut msgpack = Vec::new();
                    rec.serialize(&mut pco_pack::rmp_serde::Serializer::new(&mut msgpack).with_struct_map())?;
                    out.extend_from_slice(&(msgpack.len() as u64).to_le_bytes());
                    out.extend_from_slice(&msgpack);
                }
                Ok(out)
            }
            fn read(
                src: &mut std::io::Cursor<&[u8]>, _float_round: u32, _time_round: pco_pack::chrono::Duration,
            ) -> pco_pack::anyhow::Result<Self::Reader> {
                let buf: &[u8] = src.get_ref();
                let pos = src.position() as usize;
                if pos + 8 > buf.len() { return Err(pco_pack::anyhow::anyhow!("invalid data: expected length prefix")); }
                let bl = {
                    let mut bl_buf = [0u8; 8];
                    for i in 0..8 {
                        bl_buf[i] = buf[pos + i];
                    }
                    u64::from_le_bytes(bl_buf) as usize
                };
                src.set_position(pos as u64 + 8);
                if bl == 0 { return Err(pco_pack::anyhow::anyhow!("invalid data: zero length")); }
                let data_start = src.position() as usize;
                if data_start + bl > buf.len() { return Err(pco_pack::anyhow::anyhow!("invalid data: truncated chunk")); }
                let mp = buf[data_start..data_start + bl].to_vec();
                src.set_position((data_start + bl) as u64);
                let rec: Chunk = match pco_pack::rmp_serde::from_slice(&mp) {
                    Ok(r) => r,
                    Err(e) => return Err(pco_pack::anyhow::anyhow!("failed to deserialize chunk: {}", e)),
                };
                Ok(Reader { #(#reader_fields)* })
            }
            fn validate_bounds(reader: &mut Self::Reader) -> pco_pack::anyhow::Result<Option<usize>> {
                row_count(reader).map(Some)
            }
            fn get(reader: &mut Self::Reader, index: usize) -> pco_pack::anyhow::Result<Option<Self>> {
                let row_count = row_count(reader)?;
                if index < row_count {
                    expand_row(reader, index).map(Some)
                } else {
                    Ok(None)
                }
            }
        }
    };

    let pco_filter = quote! {
        impl pco_pack::PcoFilter for #name {
            fn filter_bulk(reader: &mut Self::Reader, field: usize, filter: &pco_pack::Filter, matches: &mut pco_pack::FilterMask) -> pco_pack::anyhow::Result<()> {
                #(#eval_filters)*
                unreachable!("filter_bulk called for unknown field index {}", field);
            }
            fn filter_nested(reader: &mut Self::Reader, path: &[usize], filter: &pco_pack::Filter, matches: &mut pco_pack::FilterMask) -> pco_pack::anyhow::Result<()> {
                match path[0] {
                    #(#nested_eval_filters)*
                    _ => unreachable!("filter_nested called with invalid path {:?}", path),
                }
            }
            fn filter_match(_value: &Self, _filter: &pco_pack::Filter) -> bool {
                false
            }
            fn resolve_filter(
                path: &str,
                json: &pco_pack::serde_json::Value,
            ) -> pco_pack::anyhow::Result<pco_pack::ResolvedFilter> {
                let (root, remainder) = match path.split_once('.') {
                    Some((head, tail)) => (head, Some(tail)),
                    None => (path, None),
                };
                match root {
                    "" => {
                        if remainder.is_some() {
                            return Err(pco_pack::anyhow::anyhow!("Empty field name cannot have a nested path"));
                        }
                        #first_column_arms
                    }
                    #(#schema_arms)*
                    _ => Err(pco_pack::anyhow::anyhow!("Field path segment '{}' does not exist in schema definition", root)),
                }
            }
        }
    };

    quote! {
        #pco_serde
        #pco_filter
    }
}
