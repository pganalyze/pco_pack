// Column vector helpers used in the generated write() loop.

use super::super::parse::{FieldRole, StructGen};
use super::super::type_helpers::{extract_timeline_const_generic, is_timeline_type};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

impl StructGen {
    /// Generate `let mut col_<field>: Vec<FieldType> = vec![];` declarations.
    pub fn col_vectors_init(&self) -> Vec<TokenStream2> {
        self.all_payload()
            .iter()
            .map(|fi| {
                let ident = &fi.ident;
                let ci = syn::Ident::new(&format!("col_{}", ident), ident.span());
                if fi.role == FieldRole::Timestamp && is_timeline_type(&fi.ty) {
                    let const_generic = extract_timeline_const_generic(&fi.ty);
                    quote! { let mut #ci: Vec<pco_pack::Timeline<#const_generic>> = Vec::new(); }
                } else {
                    let ty = &fi.ty;
                    quote! { let mut #ci: Vec<#ty> = Vec::new(); }
                }
            })
            .collect()
    }

    /// Generate `col_<field>.push(rec.<field>.clone());` calls.
    /// Works with `rec` bound as either `Self` or `&Self`.
    pub fn col_vectors_push(&self) -> Vec<TokenStream2> {
        self.all_payload()
            .iter()
            .map(|fi| {
                let ident = &fi.ident;
                let ci = syn::Ident::new(&format!("col_{}", ident), ident.span());
                quote! { #ci.push(rec .#ident.clone()); }
            })
            .collect()
    }

    /// Generate the group-key tuple expression from a reference: `&rec`.
    pub fn group_key_tuple_ref(&self) -> TokenStream2 {
        let index_fields = self.index_fields();
        if index_fields.is_empty() {
            quote! { () }
        } else if index_fields.len() == 1 {
            let ident = &index_fields[0].ident;
            quote! { (rec .#ident.clone(),) }
        } else {
            let idents: Vec<TokenStream2> = index_fields
                .iter()
                .map(|fi| {
                    let ident = &fi.ident;
                    quote! { rec .#ident.clone() }
                })
                .collect();
            quote! { (#(#idents),*) }
        }
    }
}
