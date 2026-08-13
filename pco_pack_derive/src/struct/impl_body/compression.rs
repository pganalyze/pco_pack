// Payload compression and Chunk wrapper construction.

use super::super::parse::StructGen;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

impl StructGen {
    /// Generate per-column PcoSerde::write calls that produce compressed ByteBufs.
    pub fn compress_payload(&self) -> Vec<TokenStream2> {
        let decimals = self.float_round.unwrap_or(0);
        let time_round = self.time_round.as_ref().map(|tr| quote! { #tr }).unwrap_or(quote! { Default::default() });

        self.all_payload()
            .iter()
            .map(|fi| {
                let ident = &fi.ident;
                let ty = &fi.ty;
                let ci = syn::Ident::new(&format!("col_{}", ident), ident.span());
                let cb = syn::Ident::new(&format!("cb_{}", ident), ident.span());
                quote! { let #cb = <#ty as pco_pack::PcoSerde>::write(#ci, #decimals as u32, #time_round)?; }
            })
            .collect()
    }

    /// Generate field assignments when building a Chunk from compressed columns.
    pub fn wrapper_assigns(&self) -> Vec<TokenStream2> {
        let mut assigns: Vec<TokenStream2> = Vec::new();

        for (i, fi) in self.index_fields().iter().enumerate() {
            let ident = &fi.ident;
            let idx = proc_macro2::TokenTree::from(proc_macro2::Literal::usize_unsuffixed(i));
            assigns.push(quote! { #ident: key.#idx.clone(), });
        }

        if self.has_timestamp {
            assigns.push(quote! {
                start_at: pco_pack::chrono::DateTime::<pco_pack::chrono::Utc>::from_timestamp_micros(g_start)
                    .expect("valid timestamp"),
            });
            assigns.push(quote! {
                end_at: pco_pack::chrono::DateTime::<pco_pack::chrono::Utc>::from_timestamp_micros(g_end)
                    .expect("valid timestamp"),
            });
        }

        for fi in &self.all_payload() {
            let ident = &fi.ident;
            let cb = syn::Ident::new(&format!("cb_{}", ident), ident.span());
            assigns.push(quote! { #ident: #cb.into(), });
        }

        assigns
    }
}
