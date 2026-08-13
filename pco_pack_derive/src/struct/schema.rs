use super::parse::{FieldRole, StructGen};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

impl StructGen {
    /// Generate PcoFilter match arms for each field.
    pub fn schema_arms(&self) -> Vec<TokenStream2> {
        self.field_infos
            .iter()
            .enumerate()
            .map(|(idx, fi)| {
                let ident = &fi.ident;
                let ident_str = ident.to_string();
                let ty = &fi.ty;

                if fi.role == FieldRole::Timestamp {
                    // Delegate to the type's resolve_filter (e.g., DateTime<Utc> handles
                    // integer microseconds and RFC 3339 strings).
                    quote! {
                        #ident_str => {
                            let sub_path = remainder.unwrap_or("");
                            let mut filter = <#ty as pco_pack::PcoFilter>::resolve_filter(sub_path, json)?;
                            if sub_path.is_empty() {
                                filter.path[0] = #idx;
                            } else {
                                filter.path.insert(0, #idx);
                            }
                            Ok(filter)
                        }
                    }
                } else {
                    quote! {
                        #ident_str => {
                            let sub_path = remainder.unwrap_or("");
                            let mut filter = <#ty as pco_pack::PcoFilter>::resolve_filter(sub_path, json)?;
                            if sub_path.is_empty() {
                                filter.path[0] = #idx;
                            } else {
                                filter.path.insert(0, #idx);
                            }
                            Ok(filter)
                        }
                    }
                }
            })
            .collect()
    }

    /// Generate match arms for the first column when field_name is empty.
    pub fn first_column_arms(&self) -> TokenStream2 {
        if let Some(fi) = self.field_infos.first() {
            let idx = proc_macro2::Literal::usize_unsuffixed(0);
            let ty = &fi.ty;

            if fi.role == FieldRole::Timestamp {
                quote! {
                    let sub_path = remainder.unwrap_or("");
                    let mut filter = <#ty as pco_pack::PcoFilter>::resolve_filter(sub_path, json)?;
                    if sub_path.is_empty() {
                        filter.path[0] = #idx;
                    } else {
                        filter.path.insert(0, #idx);
                    }
                    Ok(filter)
                }
            } else {
                quote! {
                    let sub_path = remainder.unwrap_or("");
                    let mut filter = <#ty as pco_pack::PcoFilter>::resolve_filter(sub_path, json)?;
                    if sub_path.is_empty() {
                        filter.path[0] = #idx;
                    } else {
                        filter.path.insert(0, #idx);
                    }
                    Ok(filter)
                }
            }
        } else {
            quote! {
                Err(pco_pack::anyhow::anyhow!("Cannot filter empty struct"))
            }
        }
    }
}
