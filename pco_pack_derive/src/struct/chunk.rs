use super::parse::StructGen;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

/// Generate expand() and filter() methods on the compressed wrapper struct.
pub fn generate(sg: &StructGen) -> TokenStream2 {
    let name = &sg.name;
    let has_timestamp = sg.has_timestamp;

    let decimals = sg.float_round.unwrap_or(0);
    let time_round = sg.time_round.as_ref().map(|tr| quote! { #tr }).unwrap_or(quote! { Default::default() });

    let index_copies: Vec<TokenStream2> = sg
        .index_fields()
        .iter()
        .map(|fi| {
            let ident = &fi.ident;
            quote! { #ident: self.#ident.clone(), }
        })
        .collect();
    let payload_wraps: Vec<TokenStream2> = sg
        .all_payload()
        .iter()
        .map(|fi| {
            let ident = &fi.ident;
            let ty = &fi.ty;
            quote! { #ident: <pco_pack::LazyReader<#ty>>::new(self.#ident.to_vec(), #decimals, #time_round), }
        })
        .collect();
    let bounds_copies = if has_timestamp {
        quote! { start_at: self.start_at, end_at: self.end_at, }
    } else {
        quote! {}
    };

    let payload_wraps_combined: Vec<TokenStream2> = sg
        .all_payload()
        .iter()
        .map(|fi| {
            let ident = &fi.ident;
            let ty = &fi.ty;
            let field_name = ident.to_string();
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
        })
        .collect();

    // Build the compute_row_count helper function body.
    // Checks payload columns in reverse declaration order (most likely to be populated),
    // but only if that column was actually loaded. Falls back to 1 if any index field
    // was requested but no payload data is available (one row per chunk with just index fields).
    let mut count_checks: TokenStream2 = quote! { 0usize };

    // Collect all payload fields first to build a match-based chain instead of nested blocks
    let mut field_checks: Vec<(proc_macro2::Ident, String)> = Vec::new();
    for fi in sg.all_payload().iter().rev() {
        let ident = &fi.ident;
        let field_name = ident.to_string();
        field_checks.push((ident.clone(), field_name));
    }

    // Build a flat iterative chain to avoid nested match expressions with ? operators.
    // Nested match+? causes exponential type-checking complexity and massive compiler memory usage.
    // Using explicit match with return Err keeps the control flow flat while propagating errors.
    if field_checks.is_empty() {
        count_checks = quote! { let count = 0usize; };
    } else {
        let mut checks = Vec::new();
        for (ident, field_name) in &field_checks {
            checks.push(quote! {
                if count == 0 && fields.contains(&#field_name) {
                    let r = reader.#ident.row_count();
                    match r {
                        Ok(n) if n > 0 => count = n,
                        Err(e) => return Err(e),
                        _ => {}
                    }
                }
            });
        }
        count_checks = quote! {
            let mut count = 0usize;
            #(#checks)*
            let count = count;
        };
    }

    let payload_field_names: Vec<_> = sg
        .all_payload()
        .iter()
        .map(|fi| {
            let name = fi.ident.to_string();
            quote! { #name }
        })
        .collect();

    let has_loaded_payload_check: TokenStream2 = if sg.all_payload().is_empty() {
        quote! { false }
    } else {
        quote! { #(
            fields.contains(&#payload_field_names) ||
        )* false }
    };

    // Compile-time constant: true if struct has any index fields.
    // Index fields are always available (uncompressed in Chunk), so if no payload
    // was loaded but index fields exist, we can return 1 row per chunk with just
    // those index fields populated.
    let has_index_fields = !sg.index_fields().is_empty();

    let compute_row_count_fn = quote! {
        fn compute_row_count(reader: &mut Reader, fields: &[&str]) -> pco_pack::anyhow::Result<usize> {
            #count_checks
            // Fallback: if no payload columns were decompressed but this struct has
            // index fields, return 1 (one row per chunk with just the index fields).
            // This handles queries that only need index fields, e.g. {"device_id": 1}.
            let has_loaded_payload = #has_loaded_payload_check;
            if count == 0 && !has_loaded_payload && #has_index_fields {
                return Ok(1);
            }
            if count == 0 {
                return Err(pco_pack::anyhow::anyhow!("Chunk has no row data and no index fields"));
            }
            Ok(count)
        }
    };

    // All field names for expand() (everything is loaded).
    let all_field_names: Vec<_> = sg
        .all_payload()
        .iter()
        .map(|fi| {
            let name = fi.ident.to_string();
            quote! { #name }
        })
        .collect();

    quote! {
        impl Chunk {
            #compute_row_count_fn

            /// Decompresses all payload columns and reconstructs the original rows.
            pub fn expand(self) -> pco_pack::anyhow::Result<Vec<#name>> {
                let mut reader = Reader {
                    #(#index_copies)*
                    #bounds_copies
                    #(#payload_wraps)*
                };
                let loaded_fields: &[&str] = &[#(#all_field_names),*];
                let row_count = Self::compute_row_count(&mut reader, loaded_fields)?;
                let mut results = Vec::with_capacity(row_count);
                for row_idx in 0..row_count {
                    results.push(expand_row(&mut reader, row_idx)?);
                }
                Ok(results)
            }

            /// Filters rows from compressed form using a JSON query.
            pub fn filter(
                &self,
                query: &pco_pack::serde_json::Value,
                fields: &[&str],
            ) -> pco_pack::anyhow::Result<Vec<#name>> {
                let fields = <#name as pco_pack::PcoPack>::resolve_fields(query, fields)?;
                let execution_plan = <#name as pco_pack::PcoFilter>::resolve_query(query)?;
                let mut reader = Reader {
                    #(#index_copies)*
                    #bounds_copies
                    #(#payload_wraps_combined)*
                };
                let row_count = Self::compute_row_count(&mut reader, &fields)?;
                let mut matches = pco_pack::FilterMask::ones(row_count);
                for step in &execution_plan {
                    <#name as pco_pack::PcoFilter>::filter_step(&mut reader, &step.path, &step.filter, &mut matches)?;
                }
                let mut results = Vec::with_capacity(matches.count_ones());
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
                        results.push(expand_row(&mut reader, actual_row_index)?);
                        current_chunk &= current_chunk - 1;
                    }
                    base_row_index += 64;
                }
                Ok(results)
            }
        }
    }
}
