// Timestamp-related helpers for the generated impl body.

use super::super::parse::{FieldRole, StructGen};
use super::super::type_helpers::{extract_timeline_const_generic, is_timeline_type};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

impl StructGen {
    /// Sort indices by the timestamp field in `data`.
    pub fn timestamp_sort_indices(&self) -> TokenStream2 {
        if self.has_timestamp {
            let rf_ident = &self.timestamp_field().unwrap().ident;
            quote! {
                indices.sort_by(|&a, &b| {
                    timestamp_to_i64(&data[a].#rf_ident)
                        .cmp(&timestamp_to_i64(&data[b].#rf_ident))
                });
            }
        } else {
            quote! {}
        }
    }

    /// Merge Timeline ranges when the timestamp is a Timeline type.
    /// Works on `indices` into `data`, materializes into `group_rows: Vec<Self>`.
    pub fn timeline_merge_from_indices(&self) -> TokenStream2 {
        if self.has_timestamp {
            let timestamp_field = match self.timestamp_field() {
                Some(rf) => rf,
                None => return quote! {},
            };

            let rf_ident = &timestamp_field.ident;

            // Non-Timeline timestamps don't need merging.
            if !is_timeline_type(&timestamp_field.ty) {
                return quote! {};
            }

            let const_generic = extract_timeline_const_generic(&timestamp_field.ty);

            // Only plain (payload) fields determine merge equality; index fields are constant within a group.
            let plain_fields: Vec<&super::super::parse::FieldInfo> =
                self.field_infos.iter().filter(|fi| matches!(fi.role, FieldRole::Plain)).collect();

            let eq_exprs: Vec<TokenStream2> = plain_fields
                .iter()
                .map(|fi| {
                    let ident = &fi.ident;
                    quote! { last.#ident == data[idx].#ident }
                })
                .collect();

            let field_inits_from_idx: Vec<TokenStream2> = self
                .field_infos
                .iter()
                .filter(|fi| !matches!(fi.role, FieldRole::Timestamp))
                .map(|fi| {
                    let ident = &fi.ident;
                    quote! { #ident: data[idx].#ident.clone(), }
                })
                .collect();

            if eq_exprs.is_empty() {
                // All rows in this group merge into one.
                quote! {
                    let mut group_rows: Vec<Self> = Vec::new();
                    if !indices.is_empty() {
                        let first_idx = indices[0];
                        let mut current = Self {
                            #rf_ident: pco_pack::Timeline::<#const_generic>::new(),
                            #(#field_inits_from_idx)*
                        };
                        for &idx in indices.iter() {
                            current.#rf_ident.extend_ranges(data[idx].#rf_ident.ranges());
                        }
                        group_rows.push(current);
                    }
                }
            } else {
                // Merge rows that share the same payload values.
                quote! {
                    let mut group_rows: Vec<Self> = Vec::new();
                    for &idx in indices.iter() {
                        if let Some(last) = group_rows.last_mut() {
                            let same = #(#eq_exprs)&&*;
                            if same {
                                last.#rf_ident.extend_ranges(data[idx].#rf_ident.ranges());
                                continue;
                            }
                        }
                        let mut ts_timeline = pco_pack::Timeline::<#const_generic>::new();
                        ts_timeline.extend_ranges(data[idx].#rf_ident.ranges());
                        group_rows.push(Self {
                            #rf_ident: ts_timeline,
                            #(#field_inits_from_idx)*
                        });
                    }
                }
            }
        } else {
            quote! {}
        }
    }

    /// Compute (g_start, g_end) timestamps from `indices` into `data` (non-chunked).
    pub fn timestamp_start_end_from_indices_var(&self, indices_var: &str) -> TokenStream2 {
        if self.has_timestamp {
            let rf_ident = &self.timestamp_field().unwrap().ident;
            let ts_field = self.timestamp_field().unwrap();
            let iv = syn::Ident::new(indices_var, proc_macro2::Span::call_site());

            if is_timeline_type(&ts_field.ty) {
                quote! {
                    let (g_start, g_end) = if !#iv.is_empty() {
                        let mut g_start = i64::MAX;
                        let mut g_end = i64::MIN;
                        for &idx in #iv.iter() {
                            if data[idx].#rf_ident.is_empty() {
                                return Err(pco_pack::anyhow::anyhow!(
                                    "empty Timeline in timestamp field `{}` is not allowed",
                                    stringify!(#rf_ident)
                                ));
                            }
                            for &(s, e) in data[idx].#rf_ident.ranges() {
                                if s < g_start { g_start = s; }
                                if e > g_end { g_end = e; }
                            }
                        }
                        (g_start, g_end)
                    } else {
                        (0i64, 0i64)
                    };
                }
            } else {
                quote! {
                    let (g_start, g_end) = if !#iv.is_empty() {
                        let first = timestamp_to_i64(&data[#iv[0]].#rf_ident);
                        let last = timestamp_to_i64(&data[#iv[#iv.len() - 1]].#rf_ident);
                        (first, last)
                    } else {
                        (0i64, 0i64)
                    };
                }
            }
        } else {
            quote! {}
        }
    }

    /// Compute (g_start, g_end) timestamps from `group_indices` into `data` (chunked).
    pub fn timestamp_start_end_from_indices(&self) -> TokenStream2 {
        self.timestamp_start_end_from_indices_var("group_indices")
    }

    /// Compute (g_start, g_end) timestamps for the chunk.
    pub fn timestamp_start_end(&self) -> TokenStream2 {
        if self.has_timestamp {
            let rf_ident = &self.timestamp_field().unwrap().ident;
            let ts_field = self.timestamp_field().unwrap();

            if is_timeline_type(&ts_field.ty) {
                quote! {
                    let (g_start, g_end) = if !group_rows.is_empty() {
                        let mut g_start = i64::MAX;
                        let mut g_end = i64::MIN;
                        for row in group_rows.iter() {
                            if row.#rf_ident.is_empty() {
                                return Err(pco_pack::anyhow::anyhow!(
                                    "empty Timeline in timestamp field `{}` is not allowed",
                                    stringify!(#rf_ident)
                                ));
                            }
                            for &(s, e) in row.#rf_ident.ranges() {
                                if s < g_start { g_start = s; }
                                if e > g_end { g_end = e; }
                            }
                        }
                        (g_start, g_end)
                    } else {
                        (0i64, 0i64)
                    };
                }
            } else {
                quote! {
                    let (g_start, g_end) = if !group_rows.is_empty() {
                        let first = timestamp_to_i64(&group_rows[0].#rf_ident);
                        let last = timestamp_to_i64(&group_rows[group_rows.len() - 1].#rf_ident);
                        (first, last)
                    } else {
                        (0i64, 0i64)
                    };
                }
            }
        } else {
            quote! {}
        }
    }

    /// Generate the timestamp_to_i64 helper function for this struct.
    pub fn timestamp_to_i64_tokens(&self) -> TokenStream2 {
        if let Some(rf) = self.timestamp_field() {
            let ty = &rf.ty;
            let ty_str = quote!(#ty).to_string();

            if ty_str.contains("DateTime") && ty_str.contains("Utc") {
                quote! {
                    #[inline] fn timestamp_to_i64(v: &#ty) -> i64 { v.timestamp_micros() }
                }
            } else if is_timeline_type(ty) {
                quote! {
                    #[inline] fn timestamp_to_i64(v: &#ty) -> i64 { v.start().unwrap_or(0) }
                }
            } else {
                quote! {
                    #[inline] fn timestamp_to_i64(v: &#ty) -> i64 { *v }
                }
            }
        } else {
            quote! {}
        }
    }

    /// Row count expression that finds the first non-empty field (handles projection).
    pub fn group_row_count_tokens(&self) -> TokenStream2 {
        if self.has_timestamp {
            let ident = self.timestamp_field().unwrap().ident.clone();
            quote! { g.#ident.row_count() }
        } else {
            let fields: Vec<&super::super::parse::FieldInfo> = self.all_payload().into_iter().collect();
            if fields.is_empty() {
                quote! { Ok(0) }
            } else if fields.len() == 1 {
                // Single field: no fallback needed
                let ident = &fields[0].ident;
                quote! { g.#ident.row_count() }
            } else {
                // Multiple fields: flat loop with explicit error propagation
                let idents: Vec<&proc_macro2::Ident> = fields.iter().map(|f| &f.ident).collect();
                let checks: Vec<TokenStream2> = idents
                    .iter()
                    .map(|ident| {
                        quote! {
                            if count == 0 {
                                match g.#ident.row_count() {
                                    Ok(n) if n > 0 => count = n,
                                    Err(e) => return Err(e),
                                    _ => {}
                                }
                            }
                        }
                    })
                    .collect();
                quote! {
                    let mut count = 0usize;
                    #(#checks)*
                    Ok(count)
                }
            }
        }
    }

    /// Generate field assignments for reconstructing a row from group data.
    pub fn expand_group_row_fields(&self) -> Vec<TokenStream2> {
        self.field_infos
            .iter()
            .map(|fi| {
                let ident = &fi.ident;
                let value = if fi.role == FieldRole::Index {
                    quote! { g.#ident.clone() }
                } else {
                    quote! { g.#ident.pop_inner(row)? }
                };
                quote! { #ident: #value, }
            })
            .collect()
    }
}
