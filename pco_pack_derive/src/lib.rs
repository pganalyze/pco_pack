mod r#enum;
mod r#struct;
#[cfg(test)]
mod test;

use proc_macro::TokenStream;
use syn::parse::{self, Parse, ParseStream};
use syn::{Data, DeriveInput, Fields, Ident, Lit, Token, parse_macro_input};

/// Parsed `#[pco_pack(...)]` attributes.
#[derive(Default)]
struct PcoPackAttrs {
    float_round: Option<u32>,
    time_round: Option<syn::Expr>,
    chunk_size: Option<usize>,
    index: Vec<Ident>,
    timestamp: Option<Ident>,
}

impl Parse for PcoPackAttrs {
    fn parse(input: ParseStream) -> parse::Result<Self> {
        let mut attrs =
            PcoPackAttrs { float_round: None, time_round: None, chunk_size: None, index: Vec::new(), timestamp: None };

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            let _: Token![=] = input.parse()?;
            match ident.to_string().as_str() {
                "timestamp" => {
                    attrs.timestamp = Some(input.parse()?);
                }
                "index" => {
                    let content;
                    syn::bracketed!(content in input);
                    attrs.index = content.parse_terminated(Ident::parse, Token![,])?.into_iter().collect();
                }
                "float_round" => {
                    // Must match pco_pack::float_round::MAX_FLOAT_ROUND_PRECISION.
                    const MAX_DECIMALS: u32 = 8;

                    if let Lit::Int(value) = Lit::parse(input)? {
                        let value = value.base10_parse::<u32>()?;
                        if value == 0 {
                            return Err(input.error("float_round must be greater than zero"));
                        }
                        if value > MAX_DECIMALS {
                            return Err(input.error(format!(
                                "float_round must be at most {MAX_DECIMALS} to avoid overflow and precision loss"
                            )));
                        }
                        attrs.float_round = Some(value);
                    } else {
                        return Err(input.error("unsupported float_round value"));
                    }
                }
                "time_round" => {
                    attrs.time_round = Some(input.parse()?);
                }
                "chunk_size" => {
                    if let Lit::Int(value) = Lit::parse(input)? {
                        let value = value.base10_parse::<usize>()?;
                        if value == 0 {
                            return Err(input.error("chunk_size must be greater than zero"));
                        }
                        attrs.chunk_size = Some(value);
                    } else {
                        return Err(input.error("unsupported chunk_size value"));
                    }
                }
                _ => {
                    return Err(input.error("unexpected ident"));
                }
            }
            let _: Option<Token![,]> = input.parse().ok();
        }

        Ok(attrs)
    }
}

#[proc_macro_derive(PcoPack, attributes(pco_pack))]
pub fn derive_pco_pack(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let attrs = match parse_pco_pack_attrs(&input.attrs) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };

    match &input.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields) => generate_struct(name, &fields.named, attrs),
            _ => syn::Error::new_spanned(&input.ident, "PcoPack can only be derived for structs with named fields")
                .to_compile_error()
                .into(),
        },
        Data::Enum(data_enum) => {
            if data_enum.variants.is_empty() {
                return syn::Error::new_spanned(&input.ident, "PcoPack cannot be derived for empty enums")
                    .to_compile_error()
                    .into();
            }
            r#enum::generate_enum_tokens(name, &data_enum.variants, &attrs).into()
        }
        _ => syn::Error::new_spanned(&input.ident, "PcoPack can only be derived for structs and enums")
            .to_compile_error()
            .into(),
    }
}

fn parse_pco_pack_attrs(attrs: &[syn::Attribute]) -> Result<PcoPackAttrs, syn::Error> {
    for attr in attrs {
        if attr.path().is_ident("pco_pack") {
            return attr.parse_args::<PcoPackAttrs>();
        }
    }
    Ok(PcoPackAttrs { float_round: None, time_round: None, chunk_size: None, index: Vec::new(), timestamp: None })
}

fn generate_struct(
    name: &syn::Ident, fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>, attrs: PcoPackAttrs,
) -> TokenStream {
    r#struct::generate(name, fields, &attrs).into()
}
