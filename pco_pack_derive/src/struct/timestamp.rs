use super::parse::StructGen;
use quote::quote;

/// Generate timestamp conversion and float_round helper functions as free functions
/// inside the anonymous const block.
pub fn generate(sg: &StructGen) -> proc_macro2::TokenStream {
    let timestamp_to_i64 = sg.timestamp_to_i64_tokens();
    let group_row_count = sg.group_row_count_tokens();
    let name = &sg.name;
    let expand_group_row_fields = sg.expand_group_row_fields();

    quote! {
        #timestamp_to_i64
        #[inline] fn row_count(g: &mut Reader) -> pco_pack::anyhow::Result<usize> {
            #group_row_count
        }
        #[inline] fn expand_row(g: &mut Reader, row: usize) -> pco_pack::anyhow::Result<#name> {
            Ok(#name { #(#expand_group_row_fields)* })
        }
    }
}
