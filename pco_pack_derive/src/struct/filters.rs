use super::parse::{FieldInfo, FieldRole, StructGen};
use super::type_helpers::*;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

impl StructGen {
    pub fn eval_filters(&self) -> Vec<TokenStream2> {
        let mut result = Vec::new();
        for (field_idx, fi) in self.field_infos.iter().enumerate() {
            let ident = &fi.ident;
            match fi.role {
                FieldRole::Index => {
                    let inner = index_filter_match(fi);
                    result.push(quote! {
                        if field == #field_idx {
                            #inner
                            return Ok(());
                        }
                    });
                }
                FieldRole::Timestamp => {
                    result.push(quote! {
                        if field == #field_idx {
                            match filter {
                                pco_pack::Filter::Range(range) => {
                                    let (start, end) = (*range.start(), *range.end());
                                    let chunk_start = reader.start_at.timestamp_micros();
                                    let chunk_end = reader.end_at.timestamp_micros();
                                    if chunk_start > end || chunk_end < start {
                                        return Ok(matches.fill(false)); // No match
                                    } else if chunk_start >= start && chunk_end <= end {
                                        return Ok(()); // Full match
                                    } else {
                                        return reader.#ident.filter_in_group(filter, matches); // Partial match
                                    }
                                }
                                _ => {
                                    return reader.#ident.filter_in_group(filter, matches);
                                }
                            }
                        }
                    });
                }
                FieldRole::Plain => {
                    result.push(quote! {
                        if field == #field_idx {
                            return reader.#ident.filter_in_group(filter, matches);
                        }
                    });
                }
            }
        }
        result
    }

    pub fn nested_eval_filters(&self) -> Vec<TokenStream2> {
        let mut result = Vec::new();
        for (field_idx, fi) in self.field_infos.iter().enumerate() {
            let ident = &fi.ident;
            match fi.role {
                FieldRole::Index => {
                    let inner = index_filter_match(fi);
                    result.push(quote! {
                        #field_idx => {
                            if path.len() > 1 {
                                return Ok(());
                            }
                            #inner
                            return Ok(());
                        }
                    });
                }
                FieldRole::Timestamp | FieldRole::Plain => {
                    result.push(quote! {
                        #field_idx => {
                            return reader.#ident.filter_nested(&path[1..], filter, matches);
                        }
                    });
                }
            }
        }
        result
    }
}

fn index_filter_match(fi: &FieldInfo) -> TokenStream2 {
    let ident = &fi.ident;
    if is_string_type(&fi.ty) {
        quote! {
            match filter {
                pco_pack::Filter::String(v) => {
                    (reader.#ident != *v).then(|| matches.fill(false));
                }
                pco_pack::Filter::InclusionString(values) => {
                    (!values.contains(&reader.#ident)).then(|| matches.fill(false));
                }
                _ => unreachable!("unexpected filter type {:?} for field {}", filter, stringify!(#ident)),
            }
        }
    } else if is_uuid_type(&fi.ty) {
        quote! {
            match filter {
                pco_pack::Filter::Uuid(v) => {
                    (reader.#ident != *v).then(|| matches.fill(false));
                }
                pco_pack::Filter::InclusionUuid(values) => {
                    (!values.contains(&reader.#ident)).then(|| matches.fill(false));
                }
                _ => unreachable!("unexpected filter type {:?} for field {}", filter, stringify!(#ident)),
            }
        }
    } else if is_bool_type(&fi.ty) {
        quote! {
            match filter {
                pco_pack::Filter::Bool(v) => {
                    (reader.#ident != *v).then(|| matches.fill(false));
                }
                pco_pack::Filter::InclusionBool(values) => {
                    (!values.contains(&reader.#ident)).then(|| matches.fill(false));
                }
                _ => unreachable!("unexpected filter type {:?} for field {}", filter, stringify!(#ident)),
            }
        }
    } else {
        let ty = &fi.ty;
        quote! {
            match filter {
                pco_pack::Filter::I64(v) => {
                    (reader.#ident != (*v as #ty)).then(|| matches.fill(false));
                }
                pco_pack::Filter::Range(range) => {
                    let val = reader.#ident as i64;
                    (!(*range.start() <= val && val <= *range.end())).then(|| matches.fill(false));
                }
                pco_pack::Filter::InclusionI64(values) => {
                    (!values.contains(&(reader.#ident as i64)))
                        .then(|| matches.fill(false));
                }
                _ => unreachable!("unexpected filter type {:?} for field {}", filter, stringify!(#ident)),
            }
        }
    }
}
