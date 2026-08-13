mod chunk;
mod filter_gen;
mod filters;
mod impl_body;
mod parse;
mod pco_pack;
mod pco_serde;
mod schema;
mod struct_defs;
mod timestamp;
mod type_helpers;

use super::PcoPackAttrs;
use parse::StructGen;
use syn::{Field, Ident, Token, punctuated::Punctuated};

pub fn generate(name: &Ident, fields: &Punctuated<Field, Token![,]>, attrs: &PcoPackAttrs) -> proc_macro2::TokenStream {
    let sg = StructGen::new(name, fields, attrs);
    let struct_defs = struct_defs::generate(&sg);
    let filter_struct = filter_gen::generate(&sg);
    let pco_serde = pco_serde::generate(&sg);
    let pco_pack = pco_pack::generate(&sg);
    let timestamp = timestamp::generate(&sg);
    let chunk = chunk::generate(&sg);

    quote::quote! {
        const _: () = {
            #[allow(unused)]
            use pco_pack::anyhow::Context;
            use pco_pack::serde::Serialize;

            #struct_defs
            #filter_struct
            #pco_serde
            #pco_pack
            #timestamp
            #chunk
        };
    }
}
