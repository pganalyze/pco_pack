use super::parse::{FieldRole, StructGen};
use super::type_helpers::is_copy_scalar;
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
            if is_copy_scalar(&fi.ty) {
                quote! { #ident: self.#ident, }
            } else {
                quote! { #ident: self.#ident.clone(), }
            }
        })
        .collect();
    let payload_wraps: Vec<TokenStream2> = sg
        .all_payload()
        .iter()
        .map(|fi| {
            let ident = &fi.ident;
            let ty = &fi.ty;
            quote! { #ident: <pco_pack::LazyReader<#ty>>::new(self.#ident.into_vec(), #decimals, #time_round), }
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

    // Sort keys for the filter plan: compressed size per column.
    // Index fields are stored uncompressed (size 0) and thus sort first.
    let mut size_keys: Vec<TokenStream2> = Vec::new();
    for fi in &sg.field_infos {
        match fi.role {
            FieldRole::Index => size_keys.push(quote! { 0 }),
            _ => {
                let ident = &fi.ident;
                size_keys.push(quote! { self.#ident.len() });
            }
        }
    }

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

    // Names `fold_into` accepts, mirroring `resolve_fields`' known set (which
    // gains start_at/end_at only for timestamped structs).
    let known_field_names: Vec<TokenStream2> = {
        let mut names = sg.all_struct_field_names.clone();
        if has_timestamp {
            names.push(quote! { "start_at" });
            names.push(quote! { "end_at" });
        }
        names
    };

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
                // Run cheap filters (small columns, index fields) before expensive ones.
                let mut plan: Vec<usize> = (0..execution_plan.len()).collect();
                let sizes = [#(#size_keys),*];
                plan.sort_by_key(|&i| sizes[execution_plan[i].path[0]]);
                for i in plan {
                    <#name as pco_pack::PcoFilter>::filter_step(&mut reader, &execution_plan[i].path, &execution_plan[i].filter, &mut matches)?;
                    // Early exit: previous filters invalidated the entire chunk.
                    if !matches.any_set_in_range(0..row_count) {
                        return Ok(Vec::new());
                    }
                }
                let mut results = Vec::with_capacity(matches.count_ones());
                matches.for_each_set_row(row_count, |actual_row_index| {
                    results.push(expand_row(&mut reader, actual_row_index)?);
                    Ok(())
                })?;
                Ok(results)
            }

            /// Validates a projection: an unloaded payload column reads back as
            /// `Default`, so an unresolved or misspelled field name must not silently
            /// become zeroed values.
            pub fn validate_projection(fields: &[&str]) -> pco_pack::anyhow::Result<()> {
                let known: &[&str] = &[#(#known_field_names),*];
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
                Ok(())
            }

            /// Streams this chunk's matching rows into a consumer, mirroring `filter`
            /// but never materializing a row vector or a full row struct. `plan` and
            /// `fields` are the outputs of `PcoFilter::resolve_query` and
            /// `PcoPack::resolve_fields`; direct callers use this one-shot path, while
            /// `for_each` resolves once per stream and reuses scratch buffers via
            /// `fold_chunk`.
            pub fn fold_into<C>(
                &self,
                plan: &[pco_pack::ResolvedFilter],
                fields: &[&str],
                consumer: &mut C,
            ) -> pco_pack::anyhow::Result<()>
            where
                C: pco_pack::Consumer<#name>,
            {
                Self::validate_projection(fields)?;
                let mut order = Vec::new();
                let mut matches = pco_pack::FilterMask::ones(0);
                self.fold_chunk(plan, fields, &mut order, &mut matches, consumer)
            }

            /// Per-chunk streaming body. `order` and `matches` are stream-scope scratch
            /// reused across chunks: the filter order depends on this chunk's column sizes,
            /// so it is re-sorted here, but neither needs a fresh allocation.
            pub fn fold_chunk<C>(
                &self,
                plan: &[pco_pack::ResolvedFilter],
                fields: &[&str],
                order: &mut Vec<usize>,
                matches: &mut pco_pack::FilterMask,
                consumer: &mut C,
            ) -> pco_pack::anyhow::Result<()>
            where
                C: pco_pack::Consumer<#name>,
            {
                let mut reader = Reader {
                    #(#index_copies)*
                    #bounds_copies
                    #(#payload_wraps_combined)*
                };
                let row_count = Self::compute_row_count(&mut reader, fields)?;
                // With nothing to filter, every row streams; skip the mask entirely.
                if plan.is_empty() {
                    let mut columns = Columns { reader: &mut reader };
                    for row_index in 0..row_count {
                        consumer.consume(&mut columns, row_index)?;
                    }
                    return Ok(());
                }
                matches.reset_ones(row_count);
                // Run cheap filters (small columns, index fields) before expensive ones.
                order.clear();
                order.extend(0..plan.len());
                let sizes = [#(#size_keys),*];
                order.sort_by_key(|&i| sizes[plan[i].path[0]]);
                for &i in order.iter() {
                    <#name as pco_pack::PcoFilter>::filter_step(&mut reader, &plan[i].path, &plan[i].filter, matches)?;
                    // Early exit: previous filters invalidated the entire chunk.
                    if !matches.any_set_in_range(0..row_count) {
                        return Ok(());
                    }
                }
                let mut columns = Columns { reader: &mut reader };
                matches.for_each_set_row(row_count, |row| consumer.consume(&mut columns, row))
            }
        }
    }
}
