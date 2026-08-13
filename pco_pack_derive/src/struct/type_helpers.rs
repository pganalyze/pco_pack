use quote::quote;

/// Match the last segment of a type path and check its identifier.
/// Returns `Some(true)` when the ident matches, `Some(false)` otherwise,
/// or `None` if the type is not a `TypePath`.
fn type_last_segment_matches(ty: &syn::Type, expected: &str) -> Option<bool> {
    if let syn::Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            return Some(seg.ident == expected);
        }
    }
    None
}

/// Check if a type is String.
pub fn is_string_type(ty: &syn::Type) -> bool {
    type_last_segment_matches(ty, "String").unwrap_or(false)
}

/// Check if a type is Uuid (from the uuid crate).
pub fn is_uuid_type(ty: &syn::Type) -> bool {
    type_last_segment_matches(ty, "Uuid").unwrap_or(false)
}

/// Check if a type is bool.
pub fn is_bool_type(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(tp) if tp.path.is_ident("bool"))
}

/// Check if a type is Timeline (for timeseries range compression).
pub fn is_timeline_type(ty: &syn::Type) -> bool {
    type_last_segment_matches(ty, "Timeline").unwrap_or(false)
}

/// Extract the const generic parameter (RESOLUTION) from a Timeline<N> type.
/// Returns the literal value as a TokenStream, or `0` if not found.
pub fn extract_timeline_const_generic(ty: &syn::Type) -> proc_macro2::TokenStream {
    if let syn::Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            if seg.ident == "Timeline" {
                if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(syn::GenericArgument::Const(const_arg)) = args.args.first() {
                        return quote! { #const_arg };
                    }
                }
            }
        }
    }
    quote! { 0 }
}
