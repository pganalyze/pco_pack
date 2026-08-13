use super::parse::StructGen;
use quote::quote;

/// Generate the PcoPack impl for the struct.
pub fn generate(sg: &StructGen) -> proc_macro2::TokenStream {
    let name = &sg.name;
    let group_key_type = &sg.group_key_type;
    let group_key_tuple_ref = sg.group_key_tuple_ref();
    let col_vectors_init = sg.col_vectors_init();
    let col_vectors_push = sg.col_vectors_push();
    let timestamp_sort_indices = sg.timestamp_sort_indices();
    let timeline_merge_from_indices = sg.timeline_merge_from_indices();
    let timestamp_start_end_from_indices = sg.timestamp_start_end_from_indices();
    let timestamp_start_end = sg.timestamp_start_end();
    let compress_payload = sg.compress_payload();
    let wrapper_assigns = sg.wrapper_assigns();
    let all_struct_field_names = &sg.all_struct_field_names;
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

    let write_body = if has_timeline_ts {
        let key_pat = if has_index {
            quote! { key }
        } else {
            quote! { _key }
        };
        quote! {
            for (#key_pat, mut indices) in groups {
                #timestamp_sort_indices
                #timeline_merge_from_indices
                for group_rows in group_rows.chunks(#name::CHUNK_SIZE) {
                    #(#col_vectors_init)*
                    for rec in group_rows {
                        #(#col_vectors_push)*
                    }
                    #(#compress_payload)*
                    #timestamp_start_end
                    let rec = Chunk { #(#wrapper_assigns)* };
                    result.push(rec);
                }
            }
        }
    } else if has_ts {
        let key_pat = if has_index {
            quote! { key }
        } else {
            quote! { _key }
        };
        quote! {
            for (#key_pat, mut indices) in groups {
                #timestamp_sort_indices
                for group_indices in indices.chunks(#name::CHUNK_SIZE) {
                    #(#col_vectors_init)*
                    for &idx in group_indices {
                        let rec = &data[idx];
                        #(#col_vectors_push)*
                    }
                    #(#compress_payload)*
                    #timestamp_start_end_from_indices
                    let rec = Chunk { #(#wrapper_assigns)* };
                    result.push(rec);
                }
            }
        }
    } else {
        let key_pat = if has_index {
            quote! { key }
        } else {
            quote! { _key }
        };
        quote! {
            for (#key_pat, indices) in groups {
                for group_indices in indices.chunks(#name::CHUNK_SIZE) {
                    #(#col_vectors_init)*
                    for &idx in group_indices {
                        let rec = &data[idx];
                        #(#col_vectors_push)*
                    }
                    #(#compress_payload)*
                    let rec = Chunk { #(#wrapper_assigns)* };
                    result.push(rec);
                }
            }
        }
    };
    let (known_fields, start_end_logic) = if let Some(ts_field) = sg.timestamp_field() {
        let ts_name = ts_field.ident.to_string();
        let known = quote! {
            let known: &[&str] = &[#(#all_struct_field_names),*, "start_at", "end_at"];
        };
        let logic = quote! {
            let ts_field = #ts_name;
            let timestamp_requested = fields.iter().any(|&f| f == ts_field)
                || query_fields.iter().any(|q| q.as_str() == ts_field);
            // Include start_at/end_at when timestamp is requested, or when all fields are requested.
            if timestamp_requested || fields.is_empty() {
                all_fields.push("start_at");
                all_fields.push("end_at");
            }
        };
        (known, logic)
    } else {
        let known = quote! {
            let known: &[&str] = &[#(#all_struct_field_names),*];
        };
        (known, quote! {})
    };
    let chunk_size = sg.chunk_size.map(|v| quote! { const CHUNK_SIZE: usize = #v; });

    quote! {
        impl pco_pack::PcoPack for #name {
            #chunk_size
            type Reader = Reader;
            type Chunk = Chunk;
            type Filter = Filter;

            fn write(data: Vec<Self>) -> pco_pack::anyhow::Result<Vec<Self::Chunk>> {
                // Group by index key using indices instead of moving Self.
                let mut groups: pco_pack::ahash::HashMap<#group_key_type, Vec<usize>> = Default::default();
                #group_enum_loop
                let mut result: Vec<Chunk> = Vec::new();
                #write_body
                Ok(result)
            }

            fn read(chunks: Vec<Self::Chunk>) -> pco_pack::anyhow::Result<Vec<Self>> {
                let mut result = Vec::new();
                for c in chunks {
                    result.extend(c.expand()?);
                }
                Ok(result)
            }

            fn to_bytes(chunks: &[Self::Chunk]) -> pco_pack::anyhow::Result<Vec<u8>> {
                let mut out = Vec::new();
                let mut ser = pco_pack::rmp_serde::Serializer::new(&mut out).with_struct_map();
                pco_pack::serde::Serialize::serialize(chunks, &mut ser)?;
                Ok(out)
            }

            fn from_bytes(bytes: &[u8]) -> pco_pack::anyhow::Result<Vec<Self::Chunk>> {
                Ok(pco_pack::rmp_serde::from_slice(bytes)?)
            }

            fn filter_value(
                chunks: &[Self::Chunk], query: &pco_pack::serde_json::Value, fields: &[&str],
            ) -> pco_pack::anyhow::Result<Vec<Self>> {
                let mut result = Vec::new();
                for c in chunks {
                    result.extend(c.filter(query, fields)?);
                }
                Ok(result)
            }

            fn resolve_fields(
                query: &pco_pack::serde_json::Value,
                fields: &[&str],
            ) -> pco_pack::anyhow::Result<Vec<&'static str>> {
                #known_fields
                // Extract top-level field names from query keys
                let query_fields: Vec<String> = match query {
                    pco_pack::serde_json::Value::Object(map) => map
                        .keys()
                        .map(|k| k.split_once('.').map(|(top, _)| top.to_string()).unwrap_or(k.clone()))
                        .collect(),
                    _ => Vec::new(),
                };
                // Validate query fields are known
                for field in &query_fields {
                    if !known.contains(&field.as_str()) {
                        return Err(pco_pack::anyhow::anyhow!("Unknown field: {}", field));
                    }
                }
                // Validate requested fields: no nested paths, all must be known
                for &field in fields {
                    if field.contains('.') {
                        let top = field.split_once('.').map(|(t, _)| t).unwrap_or(field);
                        return Err(pco_pack::anyhow::anyhow!(
                            "Nested field path '{field}' is not supported; use '{top}' instead when specifying which fields to load",
                        ));
                    }
                    if !known.contains(&field) {
                        return Err(pco_pack::anyhow::anyhow!("Unknown field: {}", field));
                    }
                }
                // Build result: only include fields that are explicitly requested
                // (in `fields`) or referenced by the query. Index/timestamp fields
                // are NOT auto-included; the caller (Chunk::filter) handles loading
                // index fields separately since they're always available uncompressed.
                let mut all_fields = Vec::new();
                if fields.is_empty() {
                    all_fields.extend(known);
                } else {
                    for &k in known {
                        if fields.contains(&k)
                            || query_fields.iter().any(|q| q.as_str() == k)
                        {
                            all_fields.push(k);
                        }
                    }
                }
                // Auto-include start_at/end_at when timestamp is requested or all fields are loaded.
                #start_end_logic
                Ok(all_fields)
            }
        }
    }
}
