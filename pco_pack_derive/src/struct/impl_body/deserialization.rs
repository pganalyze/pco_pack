// Deserialization helpers for reconstructing a Reader from a Chunk.

use super::super::parse::StructGen;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

impl StructGen {
    pub fn reader_fields(&self) -> Vec<TokenStream2> {
        let decimals = self.float_round.unwrap_or(0);
        let time_round = self.time_round.as_ref().map(|tr| quote! { #tr }).unwrap_or(quote! { Default::default() });
        let mut fields: Vec<TokenStream2> = Vec::new();
        for fi in &self.index_fields() {
            let ident = &fi.ident;
            fields.push(quote! { #ident: rec.#ident.clone(), });
        }
        if self.has_timestamp {
            fields.push(quote! { start_at: rec.start_at, });
            fields.push(quote! { end_at: rec.end_at, });
        }
        for fi in &self.all_payload() {
            let ident = &fi.ident;
            fields.push(quote! { #ident: pco_pack::LazyReader::new(rec.#ident.to_vec(), #decimals, #time_round), });
        }
        fields
    }
}
