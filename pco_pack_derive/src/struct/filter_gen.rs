use super::parse::{FieldRole, StructGen};
use quote::quote;
use syn::{Ident, Type};

/// Generate the typed Filter struct for a PcoPack-derived type.
pub fn generate(sg: &StructGen) -> proc_macro2::TokenStream {
    let mut typed_fields: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut new_params: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut new_assigns: Vec<proc_macro2::TokenStream> = Vec::new();

    // Track timestamp field for range helper methods.
    let mut ts_ident: Option<&Ident> = None;

    // Collect typed field info for TryFrom implementation.
    let mut tryfrom_entries: Vec<proc_macro2::TokenStream> = Vec::new();

    for fi in &sg.field_infos {
        match fi.role {
            FieldRole::Index => {
                let ident = &fi.ident;
                let filter_ty = field_filter_type(&fi.ty);
                typed_fields.push(quote! { pub #ident: Option<#filter_ty>, });

                new_params.push(quote! { #ident: impl Into<#filter_ty> });
                new_assigns.push(quote! { #ident: Some(#ident.into()), });

                let field_name = ident.to_string();
                tryfrom_entries.push(quote! {
                    if let Some(ref v) = value.#ident {
                        map.insert(#field_name.to_string(), pco_pack::serde_json::to_value(v)?);
                    }
                });
            }
            FieldRole::Timestamp => {
                let ident = &fi.ident;
                ts_ident = Some(ident);
                typed_fields.push(quote! { pub #ident: Option<pco_pack::DateTimeFilter>, });

                new_params.push(quote! { #ident: impl Into<pco_pack::DateTimeFilter> });
                new_assigns.push(quote! { #ident: Some(#ident.into()), });

                let field_name = ident.to_string();
                tryfrom_entries.push(quote! {
                    if let Some(ref v) = value.#ident {
                        map.insert(#field_name.to_string(), pco_pack::serde_json::to_value(v)?);
                    }
                });
            }
            FieldRole::Plain => {
                let ident = &fi.ident;
                if is_simple_type(&fi.ty) {
                    let filter_ty = field_filter_type(&fi.ty);
                    typed_fields.push(quote! { pub #ident: Option<#filter_ty>, });

                    let field_name = ident.to_string();
                    tryfrom_entries.push(quote! {
                        if let Some(ref v) = value.#ident {
                            map.insert(#field_name.to_string(), pco_pack::serde_json::to_value(v)?);
                        }
                    });
                }
                // Complex types (structs, enums, maps, tuples, Vec<T>, etc.) go into `others`.
            }
        }
    }

    let range_helpers = if let Some(ts_ident) = ts_ident {
        quote! {
            /// Returns the start and end timestamps from this filter. Requires the timestamp field to be set as a range.
            pub fn range_bounds(&self) -> pco_pack::anyhow::Result<(pco_pack::chrono::DateTime<pco_pack::chrono::Utc>, pco_pack::chrono::DateTime<pco_pack::chrono::Utc>)> {
                let filter = self.#ts_ident.as_ref().ok_or_else(|| pco_pack::anyhow::anyhow!("Timestamp missing"))?;
                filter.range_bounds()
            }

            /// Returns the duration of this filter's time range. Requires the timestamp field to be set as a range.
            pub fn range_duration(&self) -> pco_pack::anyhow::Result<pco_pack::chrono::Duration> {
                let (start, end) = self.range_bounds()?;
                Ok(end - start)
            }

            /// Shifts the filter's time range by the given duration. Requires the timestamp field to be set as a range.
            pub fn range_shift(&mut self, shift: pco_pack::chrono::Duration) -> pco_pack::anyhow::Result<()> {
                let (start, end) = self.range_bounds()?;
                self.#ts_ident = Some(pco_pack::DateTimeFilter::Range { start: start + shift, end: end + shift });
                Ok(())
            }
        }
    } else {
        quote! {}
    };

    quote! {
        /// Typed filter struct for [`#name`].
        #[derive(Clone, Default, pco_pack::serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct Filter {
            #(#typed_fields)*
            #[serde(flatten)]
            others: pco_pack::serde_json::Map<String, pco_pack::serde_json::Value>,
        }

        impl Filter {
            /// Create a new filter with the specified index and timestamp constraints.
            /// Additional fields can be set via `Index<&str>` access on the returned instance.
            pub fn new(#(#new_params),*) -> Self {
                Self {
                    #(#new_assigns)*
                    ..Default::default()
                }
            }

            /// Get a filter value for an arbitrary field (e.g., nested fields).
            pub fn get(&self, field: &str) -> Option<&pco_pack::serde_json::Value> {
                self.others.get(field)
            }

            /// Set a filter value for an arbitrary field.
            pub fn set(&mut self, field: &str, value: pco_pack::serde_json::Value) {
                self.others.insert(field.to_string(), value);
            }

            #range_helpers
        }

        impl std::ops::Index<&str> for Filter {
            type Output = pco_pack::serde_json::Value;

            fn index(&self, field: &str) -> &Self::Output {
                self.others.get(field).unwrap_or(&pco_pack::serde_json::Value::Null)
            }
        }

        impl std::ops::IndexMut<&str> for Filter {
            fn index_mut(&mut self, field: &str) -> &mut Self::Output {
                self.others.entry(field.to_string()).or_insert(pco_pack::serde_json::Value::Null)
            }
        }

        impl TryFrom<Filter> for pco_pack::serde_json::Value {
            type Error = pco_pack::anyhow::Error;

            fn try_from(value: Filter) -> Result<Self, Self::Error> {
                let mut map = pco_pack::serde_json::Map::new();
                #(#tryfrom_entries)*
                for (k, v) in &value.others {
                    map.insert(k.clone(), v.clone());
                }
                Ok(pco_pack::serde_json::Value::Object(map))
            }
        }

        impl TryFrom<pco_pack::serde_json::Value> for Filter {
            type Error = pco_pack::anyhow::Error;

            fn try_from(value: pco_pack::serde_json::Value) -> Result<Self, Self::Error> {
                Ok(pco_pack::serde_json::from_value(value)?)
            }
        }
    }
}

fn is_simple_type(ty: &Type) -> bool {
    if is_datetime_utc(ty) {
        return true;
    }

    match type_last_segment(ty).as_deref() {
        Some("i64") | Some("f64") | Some("f32")
        | Some("String") | Some("SmolStr")
        | Some("bool") | Some("Uuid")
        // Other integer types map to I64Filter.
        | Some("i32") | Some("u32") | Some("i16") | Some("u16") | Some("i8") | Some("u8") | Some("u64") => {
            true
        }
        // half::f16 is simple.
        Some("half") => true,
        _ => false,
    }
}

/// Map a field's Rust type to the appropriate typed filter enum.
fn field_filter_type(ty: &Type) -> proc_macro2::TokenStream {
    // Check for chrono::DateTime<Utc>.
    if is_datetime_utc(ty) {
        return quote! { pco_pack::DateTimeFilter };
    }

    match type_last_segment(ty).as_deref() {
        Some("i64") => quote! { pco_pack::I64Filter },
        Some("f64") | Some("f32") | Some("half") => quote! { pco_pack::F64Filter },
        Some("String") | Some("SmolStr") => quote! { pco_pack::StringFilter },
        Some("bool") => quote! { pco_pack::BoolFilter },
        Some("Uuid") => quote! { pco_pack::UuidFilter },
        // For other integer types, use I64Filter since they coerce to i64 in filters.
        Some("i32") | Some("u32") | Some("i16") | Some("u16") | Some("i8") | Some("u8") => {
            quote! { pco_pack::I64Filter }
        }
        // For u64, also use I64Filter (the runtime filter handles numeric coercion).
        Some("u64") => quote! { pco_pack::I64Filter },
        _ => {
            // Fallback: for unknown types, users must set via Index<&str> with JSON.
            // We still generate a typed field but use serde_json::Value as the filter type.
            quote! { pco_pack::serde_json::Value }
        }
    }
}

/// Match the last segment of a type path and return its identifier.
fn type_last_segment(ty: &Type) -> Option<String> {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            return Some(seg.ident.to_string());
        }
    }
    None
}

/// Check if a type is chrono::DateTime<Utc>.
fn is_datetime_utc(ty: &Type) -> bool {
    if let Type::Path(tp) = ty {
        if let Some(last_seg) = tp.path.segments.last() {
            if last_seg.ident == "DateTime" {
                if let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments {
                    // Check for <Utc> generic argument.
                    for arg in &args.args {
                        if let syn::GenericArgument::Type(Type::Path(gtp)) = arg {
                            if let Some(seg) = gtp.path.segments.last() {
                                if seg.ident == "Utc" {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    false
}
