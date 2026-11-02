// Staging columns for the generated write loop.

use super::super::parse::StructGen;
use super::super::type_helpers::is_copy_scalar;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

impl StructGen {
    /// Generate the staging declarations for one field: an owned column
    /// and a reference column. The `CHEAP_TO_CLONE` guard folds per
    /// monomorphized type, so the unused column stays empty and allocates
    /// nothing.
    pub fn col_vectors_init(&self) -> Vec<TokenStream2> {
        self.all_payload()
            .iter()
            .map(|fi| {
                let ident = &fi.ident;
                let ty = &fi.ty;
                let ci = syn::Ident::new(&format!("col_{}", ident), ident.span());
                let cr = syn::Ident::new(&format!("colr_{}", ident), ident.span());
                quote! {
                    let mut #ci: Vec<#ty> = Vec::new();
                    let mut #cr: Vec<& #ty> = Vec::new();
                }
            })
            .collect()
    }

    /// Generate the per-field staging push. Cheap types land by value
    /// (copy scalars, clone the rest); expensive types stage only a
    /// reference into the row. The const guard folds at compile time.
    pub fn col_vectors_push(&self) -> Vec<TokenStream2> {
        self.all_payload()
            .iter()
            .map(|fi| {
                let ident = &fi.ident;
                let ty = &fi.ty;
                let ci = syn::Ident::new(&format!("col_{}", ident), ident.span());
                let cr = syn::Ident::new(&format!("colr_{}", ident), ident.span());
                let owned_value = if is_copy_scalar(ty) {
                    quote! { rec .#ident }
                } else {
                    quote! { rec .#ident .clone() }
                };
                quote! {
                    if <#ty as pco_pack::PcoSerde>::CHEAP_TO_CLONE {
                        #ci.push(#owned_value);
                    } else {
                        #cr.push(&rec .#ident);
                    }
                }
            })
            .collect()
    }

    /// Generate the conjunction of per-field `CHEAP_TO_CLONE` constants,
    /// so a struct with an expensive field is itself expensive. Repeated
    /// types collapse to one term (clippy's eq_op rejects duplicates).
    pub fn cheap_to_clone_expr(&self) -> TokenStream2 {
        let mut seen = std::collections::HashSet::new();
        let mut terms = self
            .field_infos
            .iter()
            .map(|fi| {
                let ty = &fi.ty;
                quote! { <#ty as pco_pack::PcoSerde>::CHEAP_TO_CLONE }
            })
            .filter(|term| seen.insert(term.to_string()));
        match terms.next() {
            Some(first) => quote! { #first #(&& #terms)* },
            None => quote! { true },
        }
    }

    /// Generate the group-key tuple expression from a reference: `&rec`.
    pub fn group_key_tuple_ref(&self) -> TokenStream2 {
        let index_fields = self.index_fields();
        if index_fields.is_empty() {
            quote! { () }
        } else if index_fields.len() == 1 {
            let ident = &index_fields[0].ident;
            if is_copy_scalar(&index_fields[0].ty) {
                quote! { (rec .#ident, ) }
            } else {
                quote! { (rec .#ident.clone(),) }
            }
        } else {
            let idents: Vec<TokenStream2> = index_fields
                .iter()
                .map(|fi| {
                    let ident = &fi.ident;
                    if is_copy_scalar(&fi.ty) {
                        quote! { rec .#ident }
                    } else {
                        quote! { rec .#ident.clone() }
                    }
                })
                .collect();
            quote! { (#(#idents),*) }
        }
    }
}
