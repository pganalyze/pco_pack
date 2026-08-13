use super::PcoPackAttrs;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Ident, punctuated::Punctuated, token::Comma};

/// All intermediate parsed state for a struct derive.
pub struct StructGen {
    pub name: Ident,
    pub field_infos: Vec<FieldInfo>,
    pub has_timestamp: bool,
    pub float_round: Option<u32>,
    pub time_round: Option<TokenStream2>,
    pub chunk_size: Option<usize>,
    pub group_key_type: TokenStream2,
    pub all_struct_field_names: Vec<TokenStream2>,
}

/// Role of a struct field in the PcoPack schema.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FieldRole {
    Index,
    Timestamp,
    Plain,
}

/// Parsed information about a single struct field.
pub struct FieldInfo {
    pub ident: Ident,
    pub ty: syn::Type,
    pub role: FieldRole,
}

impl StructGen {
    pub fn index_fields(&self) -> Vec<&FieldInfo> {
        self.field_infos.iter().filter(|f| matches!(f.role, FieldRole::Index)).collect()
    }

    pub fn timestamp_field(&self) -> Option<&FieldInfo> {
        self.field_infos.iter().find(|f| matches!(f.role, FieldRole::Timestamp))
    }

    pub fn payload_fields(&self) -> Vec<&FieldInfo> {
        self.field_infos
            .iter()
            .filter(|f| !matches!(f.role, FieldRole::Index) && !matches!(f.role, FieldRole::Timestamp))
            .collect()
    }

    pub fn all_payload(&self) -> Vec<&FieldInfo> {
        let mut all_payload: Vec<&FieldInfo> = Vec::new();
        if let Some(rf) = self.timestamp_field() {
            all_payload.push(rf);
        }
        all_payload.extend(self.payload_fields().into_iter());
        all_payload
    }

    pub fn new(name: &Ident, fields: &Punctuated<syn::Field, Comma>, attrs: &PcoPackAttrs) -> Self {
        let index_set: Vec<_> = attrs.index.iter().collect();
        let float_round = attrs.float_round;
        let time_round = attrs.time_round.as_ref().map(|expr| quote::quote!(#expr));
        let chunk_size = attrs.chunk_size;
        let field_infos = classify_fields(fields, &index_set, attrs.timestamp.as_ref());
        let has_timestamp = attrs.timestamp.is_some();

        let index_fields = field_infos.iter().filter(|f| matches!(f.role, FieldRole::Index)).collect::<Vec<_>>();
        let group_key_type = if index_fields.is_empty() {
            quote! { () }
        } else if index_fields.len() == 1 {
            let ty = &index_fields[0].ty;
            quote! { (#ty,) }
        } else {
            let types: Vec<_> = index_fields
                .iter()
                .map(|fi| {
                    let ty = &fi.ty;
                    quote! { #ty }
                })
                .collect();
            quote! { (#(#types),*) }
        };

        // All struct field names in declaration order (used by resolve_fields)
        let all_struct_field_names: Vec<TokenStream2> = field_infos
            .iter()
            .map(|fi| {
                let name = fi.ident.to_string();
                quote! { #name }
            })
            .collect();

        StructGen {
            name: name.clone(),
            field_infos,
            has_timestamp,
            float_round,
            time_round,
            chunk_size,
            group_key_type,
            all_struct_field_names,
        }
    }
}

fn classify_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>, index_set: &[&Ident],
    timestamp_field_name: Option<&Ident>,
) -> Vec<FieldInfo> {
    fields
        .iter()
        .map(|f| {
            let ident = f.ident.as_ref().unwrap();
            let ty = &f.ty;
            let role = if index_set.contains(&ident) {
                FieldRole::Index
            } else if Some(ident) == timestamp_field_name {
                FieldRole::Timestamp
            } else {
                FieldRole::Plain
            };
            FieldInfo { ident: ident.clone(), ty: ty.clone(), role }
        })
        .collect()
}
