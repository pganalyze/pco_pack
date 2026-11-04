//! `fold!` expands to a block defining a uniquely named `PcoFold{N}` struct that wraps
//! the closures and implements `pco_pack::Consumer<Ty>` for them; the closure body may
//! return `()` or `Result<()>`. User-facing syntax is documented on the `fold` macro
//! itself; this module records the generator constraints.
//!
//! Struct fold parameters always carry type annotations: the generated `where` clauses
//! need concrete argument types, since a generic impl body cannot type-check calling a
//! closure with concrete accessor results. The annotations double as the closure's
//! parameter types. Accessors resolve by parameter name, so a mistyped annotation
//! fails against the real column type, and `FIELDS` agrees with the parameters by
//! construction. Chunk bounds (`start_at`/`end_at`) belong to the chunk, not to a row
//! column, so they reach consumers only through hydrated rows.
//!
//! A tuple-struct pattern element cannot carry a type annotation (neither rustc nor
//! syn parses `Tick(value: u64)`), so a payload binding's annotation follows the
//! pattern. Struct folds annotate every parameter, which means a bare unannotated
//! single parameter is always a variant name; single- and multi-arm forms spell
//! variants identically. syn spells a bare name `Pat::Ident` and only qualified paths
//! `Pat::Path`.
//!
//! Existence is checked at compile time: every pattern variant references a
//! per-variant const the derive emits (`Ty::__pco_variant_Tick`), so a misspelled
//! variant, a qualified pattern naming a different type, or a unit pattern on a
//! struct fails to build instead of failing at runtime.

use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident, quote, quote_spanned};
use std::sync::atomic::{AtomicUsize, Ordering};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{ExprClosure, Ident, Index, Pat, PatType, Token};

/// Counts `fold!` expansions so generated struct names stay unique across repeats in
/// one scope.
static FOLD_SEQ: AtomicUsize = AtomicUsize::new(0);

const HELP_ANNOT: &str = "annotate each payload field with its column type, e.g. `value: f64`";
const HELP_VARIANT: &str = "each closure of a multi-variant fold must name a variant pattern, e.g. `fold!(Job, |Idle| ..., |Tick(Reading { value: u64 })| ...)`";

pub struct FoldInput {
    ty: Ident,
    closures: Punctuated<ExprClosure, Token![,]>,
}

impl Parse for FoldInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ty: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let closures = Punctuated::parse_terminated(input)?;
        if closures.is_empty() {
            return Err(input.error("fold! needs at least one closure"));
        }
        Ok(Self { ty, closures })
    }
}

#[derive(Clone)]
struct FoldField {
    ident: Ident,
    ty: syn::Type,
}

/// How one payload arm's closure reads its variant: selected fields popped from a
/// projected view of the payload block, or the whole payload value bound by
/// `PcoSerde::get`. Both advance the variant's slot cursor exactly once per row.
enum PayloadKind {
    /// `Tick(Reading { value: f64 })`
    Fields { payload: syn::Path, fields: Vec<FoldField> },
    /// `Tick(value): u64`
    Bind { field: FoldField },
}

/// One arm of an enum fold: the variant it matches and what its closure receives.
enum Arm {
    /// Unit variant, or a payload variant folded for membership only.
    Unit { variant: Ident },
    /// Payload variant whose closure reads the payload, by field or whole.
    Payload { variant: Ident, kind: Box<PayloadKind> },
}

impl Arm {
    fn variant(&self) -> &Ident {
        match self {
            Arm::Unit { variant } | Arm::Payload { variant, .. } => variant,
        }
    }
}

impl PayloadKind {
    fn fields(&self) -> Vec<FoldField> {
        match self {
            PayloadKind::Fields { fields, .. } => fields.clone(),
            PayloadKind::Bind { field } => vec![field.clone()],
        }
    }
}

/// Resolves a pattern path to its variant ident. A qualified path must name the
/// folded type, so `Other::Tick` inside `fold!(Job, ...)` is a compile error rather
/// than a fold against the wrong enum's same-named variant.
fn pattern_variant(ty: &Ident, path: &syn::Path, span: proc_macro2::Span) -> syn::Result<Ident> {
    let segments: Vec<_> = path.segments.iter().collect();
    match segments.as_slice() {
        [only] => Ok(only.ident.clone()),
        [first, last] if first.ident == *ty => Ok(last.ident.clone()),
        _ => Err(syn::Error::new(
            span,
            format!(
                "`{}` does not name a variant of `{ty}`; write the variant bare (`Tick`) or qualified as `{ty}::Tick`",
                path.to_token_stream(),
            ),
        )),
    }
}

/// Item referencing the derive's per-variant existence const. Compiles only when the
/// variant exists, and the probe ident carries the user's span so the error points at
/// the pattern.
fn variant_probe(ty: &Ident, variant: &Ident) -> TokenStream2 {
    let probe = format_ident!("__pco_variant_{}", variant, span = variant.span());
    quote! { const _: () = #ty::#probe; }
}

pub(crate) fn expand(input: FoldInput) -> syn::Result<TokenStream2> {
    let FoldInput { ty, closures } = input;
    let mut closures: Vec<ExprClosure> = closures.into_iter().collect();
    if closures.len() == 1 {
        expand_single(ty, closures.pop().expect("len checked"))
    } else {
        expand_multi(ty, closures)
    }
}

fn expand_single(ty: Ident, closure: ExprClosure) -> syn::Result<TokenStream2> {
    // Enum forms first: a single annotated parameter is a struct fold, so a bare
    // unannotated one is a variant name and never a struct fold.
    if closure.inputs.len() == 1 {
        // Extract the pattern shape into owned data; the dispatch moves `closure`
        // itself into the chosen expansion.
        enum Form {
            Variant(Box<(syn::PatTupleStruct, Option<syn::Type>)>),
            Unit(Ident),
            Struct,
        }
        let form = match closure.inputs.iter().next().expect("len checked") {
            Pat::TupleStruct(ts) => Form::Variant(Box::new((ts.clone(), None))),
            Pat::Type(pt) => match &*pt.pat {
                Pat::TupleStruct(ts) => Form::Variant(Box::new((ts.clone(), Some(*pt.ty.clone())))),
                Pat::Path(p) if p.qself.is_none() => {
                    return Err(syn::Error::new(pt.span(), "a unit pattern binds nothing; drop the type annotation"));
                }
                _ => Form::Struct,
            },
            Pat::Path(p) if p.qself.is_none() => Form::Unit(pattern_variant(&ty, &p.path, p.span())?),
            Pat::Ident(pi) if pi.subpat.is_none() && pi.by_ref.is_none() && pi.mutability.is_none() => {
                Form::Unit(pi.ident.clone())
            }
            _ => Form::Struct,
        };
        match form {
            Form::Variant(inner) => {
                let (ts, annot) = *inner;
                return expand_variant(ty, closure, &ts, annot);
            }
            Form::Unit(variant) => return expand_unit_variant(ty, closure, variant),
            Form::Struct => {}
        }
    }

    let help =
        "annotate each fold! parameter with its column type, e.g. `fold!(Row, |fingerprint: u64, calls: u32| ...)`";
    let mut fields = Vec::new();
    for pat in &closure.inputs {
        match pat {
            Pat::Type(PatType { pat, ty: field_ty, .. }) => match pat.as_ref() {
                Pat::Ident(syn::PatIdent { subpat: None, by_ref: None, .. }) => {
                    let Pat::Ident(pat_ident) = pat.as_ref() else { unreachable!("matched above") };
                    fields.push(FoldField { ident: pat_ident.ident.clone(), ty: (**field_ty).clone() });
                }
                _ => return Err(syn::Error::new(pat.span(), help)),
            },
            _ => return Err(syn::Error::new(pat.span(), help)),
        }
    }
    if fields.is_empty() {
        return Err(syn::Error::new(
            closure.span(),
            "fold! needs at least one annotated field parameter or a variant pattern; iterate hydrated rows by passing an annotated `|row: Ty|` closure to `for_each`",
        ));
    }

    let names: Vec<syn::LitStr> =
        fields.iter().map(|f| syn::LitStr::new(&f.ident.to_string(), f.ident.span())).collect();
    let arg_tys: Vec<&syn::Type> = fields.iter().map(|f| &f.ty).collect();
    // Span pops with the user's annotation so a mistyped column fails there, not in
    // the expansion.
    let pops: Vec<TokenStream2> = fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            quote_spanned! { f.ty.span() => __pco_cols.#ident(__pco_row)? }
        })
        .collect();

    // Two expansions with the same fields and types emit identical impl headers and
    // would collide, so each gets its own consumer struct.
    let seq = FOLD_SEQ.fetch_add(1, Ordering::Relaxed);
    let sel = format_ident!("PcoFold{}", seq);

    Ok(quote! {
        {
            #[doc(hidden)]
            struct #sel<__PcoF>(__PcoF);

            impl<__PcoF, __PcoO> pco_pack::Consumer<#ty> for #sel<__PcoF>
            where
                __PcoF: FnMut(#(#arg_tys,)*) -> __PcoO,
                __PcoO: pco_pack::Output,
            {
                const FIELDS: &'static [&'static str] = &[#(#names),*];

                fn consume(
                    &mut self,
                    __pco_cols: &mut <#ty as pco_pack::PcoPack>::Columns<'_>,
                    __pco_row: usize,
                ) -> pco_pack::anyhow::Result<()> {
                    pco_pack::Output::lift((self.0)( #(#pops),* ))
                }
            }

            #sel(#closure)
        }
    })
}

/// Parses the annotated field list of a struct payload pattern, e.g. the `value: f64`
/// of `Tick(Reading { value: f64 })`. Rejects `..` and shorthand bindings, which carry
/// no column type to project on.
fn payload_fields(view: &syn::PatStruct) -> syn::Result<Vec<FoldField>> {
    let help = "an enum fold names a variant and its annotated payload fields, e.g. `fold!(Job, |Tick(Reading { value: f64 })| ...)`";
    if view.fields.is_empty() {
        return Err(syn::Error::new(view.span(), help));
    }
    let mut fields = Vec::new();
    for field in &view.fields {
        if matches!(&*field.pat, Pat::Rest(_)) {
            return Err(syn::Error::new(
                field.pat.span(),
                "list every field the closure reads; `..` cannot be projected",
            ));
        }
        let syn::Member::Named(name) = &field.member else {
            return Err(syn::Error::new(field.span(), "enum folds select named payload fields"));
        };
        // Qualified or generic types (`Vec<u8>`) parse as Pat::Path; a bare name equal
        // to the field is shorthand binding syntax, carrying no column type.
        let annot: &syn::Path = match &*field.pat {
            Pat::Path(p) if p.qself.is_none() => &p.path,
            Pat::Ident(pi) if pi.subpat.is_none() && pi.by_ref.is_none() && pi.mutability.is_none() => {
                if pi.ident == *name {
                    return Err(syn::Error::new(
                        field.pat.span(),
                        format!("`{name}` is a binding, not a column type; {HELP_ANNOT}"),
                    ));
                }
                let field_ty = syn::Type::Path(syn::TypePath { qself: None, path: pi.ident.clone().into() });
                fields.push(FoldField { ident: name.clone(), ty: field_ty });
                continue;
            }
            _ => return Err(syn::Error::new(field.pat.span(), HELP_ANNOT)),
        };
        let field_ty: syn::Type =
            syn::parse2(annot.to_token_stream()).map_err(|_| syn::Error::new(annot.span(), HELP_ANNOT))?;
        fields.push(FoldField { ident: name.clone(), ty: field_ty });
    }
    Ok(fields)
}

/// Parses a payload element: a struct pattern selecting fields
/// (`Tick(Reading { value: f64 })`) or a plain binding whose type was annotated after
/// the pattern (`Tick(value): u64`).
fn payload_kind(elem: &Pat, annot: Option<syn::Type>) -> syn::Result<PayloadKind> {
    let help = "an enum fold's payload binding is either an annotated value, e.g. `Tick(value): u64`, \
                or a struct pattern selecting fields, e.g. `Tick(Reading { value: f64 })`";
    match elem {
        Pat::Struct(view) => {
            if annot.is_some() {
                return Err(syn::Error::new(
                    elem.span(),
                    "the struct pattern form already names the payload type; drop the annotation",
                ));
            }
            Ok(PayloadKind::Fields { payload: view.path.clone(), fields: payload_fields(view)? })
        }
        Pat::Ident(syn::PatIdent { ident, subpat: None, by_ref: None, mutability: None, .. }) => match annot {
            Some(ty) => Ok(PayloadKind::Bind { field: FoldField { ident: ident.clone(), ty } }),
            None => Err(syn::Error::new(
                elem.span(),
                format!(
                    "`{ident}` is a binding with no type; write the payload type after the pattern, e.g. `Tick({ident}): u64`"
                ),
            )),
        },
        _ => Err(syn::Error::new(elem.span(), format!("{help}; the binding must be a plain name"))),
    }
}

/// Rewrites a variant pattern to flat annotated bindings so the generated call site
/// keeps one argument per projected field; the annotations double as the closure's
/// parameter types.
fn rewrite_inputs(closure: &mut ExprClosure, fields: &[FoldField]) {
    closure.inputs = fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            let ty = &f.ty;
            Pat::Type(PatType {
                attrs: Vec::new(),
                pat: Box::new(Pat::Ident(syn::PatIdent {
                    attrs: Vec::new(),
                    by_ref: None,
                    mutability: None,
                    ident: ident.clone(),
                    subpat: None,
                })),
                colon_token: <Token![:]>::default(),
                ty: Box::new(ty.clone()),
            })
        })
        .collect();
}

/// `consume`/`consume_variant` body for a payload arm: take the variant's lazy block
/// and slot from the enum column view (one cursor advance per row), then read the
/// arguments field-by-field through a projected payload view or whole through
/// `PcoSerde::get`. `closure_ref` is the expression calling the closure.
fn consume_body(kind: &PayloadKind, view_fn: &Ident, closure_ref: TokenStream2) -> TokenStream2 {
    match kind {
        PayloadKind::Fields { payload, fields } => {
            let pops: Vec<TokenStream2> = fields
                .iter()
                .map(|f| {
                    let ident = &f.ident;
                    quote_spanned! { f.ty.span() => __pco_view.#ident(__pco_slot)? }
                })
                .collect();
            quote! {
                let (__pco_block, __pco_slot) = __pco_cols.#view_fn(__pco_row)?;
                let mut __pco_view = <#payload as pco_pack::PcoPack>::columns(__pco_block.reader_mut()?);
                pco_pack::Output::lift(#closure_ref( #(#pops),* ))
            }
        }
        PayloadKind::Bind { field } => {
            let arg_ty = &field.ty;
            quote! {
                let (__pco_block, __pco_slot) = __pco_cols.#view_fn(__pco_row)?;
                let __pco_value = <#arg_ty as pco_pack::PcoSerde>::get(__pco_block.reader_mut()?, __pco_slot)?
                    .ok_or_else(|| pco_pack::anyhow::anyhow!(
                        "payload block ended before slot {} of the folded rows",
                        __pco_slot
                    ))?;
                pco_pack::Output::lift(#closure_ref(__pco_value))
            }
        }
    }
}

/// Single payload-variant pattern. The generated `consume` asks the enum column view
/// for the variant's block and reads at the row's payload slot; the slot cursor
/// advances exactly once per row.
fn expand_variant(
    ty: Ident, mut closure: ExprClosure, ts: &syn::PatTupleStruct, annot: Option<syn::Type>,
) -> syn::Result<TokenStream2> {
    let variant = pattern_variant(&ty, &ts.path, ts.span())?;
    if ts.elems.is_empty() {
        if annot.is_some() {
            return Err(syn::Error::new(ts.span(), "a unit pattern binds nothing; drop the type annotation"));
        }
        // `|Tick()|` on any variant: no payload to bind, count membership only.
        return expand_unit_variant(ty, closure, variant);
    }
    if ts.elems.len() != 1 {
        return Err(syn::Error::new(
            ts.span(),
            "an enum fold's variant must have exactly one payload binding, e.g. `Tick(Reading { value: f64 })`",
        ));
    }
    let kind = payload_kind(ts.elems.iter().next().unwrap(), annot)?;

    let variant_lit = syn::LitStr::new(&variant.to_string(), variant.span());
    let view_fn = format_ident!("variant_{}", variant);
    let probe = variant_probe(&ty, &variant);
    let body = consume_body(&kind, &view_fn, quote! { (self.0) });
    let arg_tys: Vec<syn::Type> = kind.fields().into_iter().map(|f| f.ty).collect();
    rewrite_inputs(&mut closure, &kind.fields());

    let seq = FOLD_SEQ.fetch_add(1, Ordering::Relaxed);
    let sel = format_ident!("PcoFold{}", seq);

    Ok(quote! {
        {
            #probe

            #[doc(hidden)]
            struct #sel<__PcoF>(__PcoF);

            impl<__PcoF, __PcoO> pco_pack::Consumer<#ty> for #sel<__PcoF>
            where
                __PcoF: FnMut(#(#arg_tys,)*) -> __PcoO,
                __PcoO: pco_pack::Output,
            {
                const FIELDS: &'static [&'static str] = &[#variant_lit];
                #[doc(hidden)]
                const POPS_PAYLOAD: &'static [bool] = &[true];

                fn consume(
                    &mut self,
                    __pco_cols: &mut <#ty as pco_pack::PcoPack>::Columns<'_>,
                    __pco_row: usize,
                ) -> pco_pack::anyhow::Result<()> {
                    #body
                }
            }

            #sel(#closure)
        }
    })
}

/// Unit-variant pattern (membership fold): the engine narrows the mask to the variant
/// and calls a zero-argument closure per row. Nothing is popped, and the count comes
/// from the discriminant without decoding the payload block, so this arm stays off the
/// slot fast path (`POPS_PAYLOAD` false).
fn expand_unit_variant(ty: Ident, mut closure: ExprClosure, variant: Ident) -> syn::Result<TokenStream2> {
    // The body cannot reference the variant pattern, so drop the parameter.
    closure.inputs.clear();

    let variant_lit = syn::LitStr::new(&variant.to_string(), variant.span());
    let probe = variant_probe(&ty, &variant);
    let seq = FOLD_SEQ.fetch_add(1, Ordering::Relaxed);
    let sel = format_ident!("PcoFold{}", seq);

    Ok(quote! {
        {
            #probe

            #[doc(hidden)]
            struct #sel<__PcoF>(__PcoF);

            impl<__PcoF, __PcoO> pco_pack::Consumer<#ty> for #sel<__PcoF>
            where
                __PcoF: FnMut() -> __PcoO,
                __PcoO: pco_pack::Output,
            {
                const FIELDS: &'static [&'static str] = &[#variant_lit];
                #[doc(hidden)]
                const POPS_PAYLOAD: &'static [bool] = &[false];

                fn consume(
                    &mut self,
                    _cols: &mut <#ty as pco_pack::PcoPack>::Columns<'_>,
                    _row: usize,
                ) -> pco_pack::anyhow::Result<()> {
                    pco_pack::Output::lift((self.0)())
                }
            }

            #sel(#closure)
        }
    })
}

/// Parses one multi-arm closure: an empty pattern or bare path/ident is
/// membership-only; one element is a payload binding, optionally annotated after the
/// pattern.
fn payload_arm(ty: &Ident, ts: &syn::PatTupleStruct, annot: Option<syn::Type>) -> syn::Result<Arm> {
    let variant = pattern_variant(ty, &ts.path, ts.span())?;
    if ts.elems.is_empty() {
        if annot.is_some() {
            return Err(syn::Error::new(ts.span(), "a unit pattern binds nothing; drop the type annotation"));
        }
        return Ok(Arm::Unit { variant });
    }
    if ts.elems.len() != 1 {
        return Err(syn::Error::new(
            ts.span(),
            "an enum fold's variant must have exactly one payload binding, e.g. `Tick(Reading { value: f64 })`",
        ));
    }
    let kind = payload_kind(ts.elems.iter().next().unwrap(), annot)?;
    Ok(Arm::Payload { variant, kind: Box::new(kind) })
}

/// Multi-arm form: one closure per declared variant. `FIELDS` lists variant names in
/// arm order, which doubles as the `consume_variant` arm index; `fold!` sees variant
/// names only, so the engine maps each row's discriminant to its arm.
fn expand_multi(ty: Ident, closures: Vec<ExprClosure>) -> syn::Result<TokenStream2> {
    let mut arms: Vec<(Arm, ExprClosure)> = Vec::with_capacity(closures.len());
    let mut seen: Vec<String> = Vec::with_capacity(closures.len());
    for mut closure in closures {
        if closure.inputs.len() != 1 {
            return Err(syn::Error::new(closure.span(), HELP_VARIANT));
        }
        let pat = closure.inputs.iter().next().expect("len checked");
        let arm = match pat {
            Pat::TupleStruct(ts) => payload_arm(&ty, ts, None)?,
            Pat::Type(pt) => match &*pt.pat {
                Pat::TupleStruct(ts) => payload_arm(&ty, ts, Some(*pt.ty.clone()))?,
                _ => return Err(syn::Error::new(pat.span(), HELP_VARIANT)),
            },
            Pat::Path(p) if p.qself.is_none() => Arm::Unit { variant: pattern_variant(&ty, &p.path, p.span())? },
            Pat::Ident(pi) if pi.subpat.is_none() && pi.by_ref.is_none() && pi.mutability.is_none() => {
                Arm::Unit { variant: pi.ident.clone() }
            }
            _ => return Err(syn::Error::new(pat.span(), HELP_VARIANT)),
        };
        let name = arm.variant().to_string();
        if seen.contains(&name) {
            return Err(syn::Error::new(arm.variant().span(), format!("variant `{name}` declared twice")));
        }
        seen.push(name);
        match &arm {
            Arm::Payload { kind, .. } => rewrite_inputs(&mut closure, &kind.fields()),
            Arm::Unit { .. } => closure.inputs.clear(),
        }
        arms.push((arm, closure));
    }

    let generic_fns: Vec<Ident> = (0..arms.len()).map(|i| format_ident!("__PcoF{}", i)).collect();
    let generic_outs: Vec<Ident> = (0..arms.len()).map(|i| format_ident!("__PcoO{}", i)).collect();
    let name_lits: Vec<syn::LitStr> =
        arms.iter().map(|(arm, _)| syn::LitStr::new(&arm.variant().to_string(), arm.variant().span())).collect();
    let probes: Vec<Ident> = arms
        .iter()
        .map(|(arm, _)| format_ident!("__pco_variant_{}", arm.variant(), span = arm.variant().span()))
        .collect();
    let pops: Vec<bool> = arms.iter().map(|(arm, _)| matches!(arm, Arm::Payload { .. })).collect();
    // Only payload arms read the row argument; an all-unit fold needs the underscore
    // name to stay warning-free.
    let row_ident = if pops.iter().any(|p| *p) { format_ident!("__pco_row") } else { format_ident!("_row") };

    let where_clauses: Vec<TokenStream2> = generic_fns
        .iter()
        .zip(generic_outs.iter())
        .zip(arms.iter())
        .map(|((gf, go), (arm, _))| {
            let arg_tys: Vec<syn::Type> = match arm {
                Arm::Unit { .. } => Vec::new(),
                Arm::Payload { kind, .. } => kind.fields().into_iter().map(|f| f.ty).collect(),
            };
            quote! { #gf: FnMut(#(#arg_tys,)*) -> #go, #go: pco_pack::Output }
        })
        .collect();

    let match_arms: Vec<TokenStream2> = arms
        .iter()
        .enumerate()
        .map(|(i, (arm, _))| {
            let arm_idx = Index::from(i);
            match arm {
                Arm::Unit { .. } => quote! {
                    #arm_idx => pco_pack::Output::lift((self.#arm_idx)()),
                },
                Arm::Payload { variant, kind } => {
                    let view_fn = format_ident!("variant_{}", variant);
                    let closure_ref = quote! { (self.#arm_idx) };
                    let body = consume_body(kind, &view_fn, closure_ref);
                    quote! { #arm_idx => { #body } }
                }
            }
        })
        .collect();

    let rewritten: Vec<ExprClosure> = arms.into_iter().map(|(_, closure)| closure).collect();
    let seq = FOLD_SEQ.fetch_add(1, Ordering::Relaxed);
    let sel = format_ident!("PcoFold{}", seq);

    Ok(quote! {
        {
            #(const _: () = #ty::#probes;)*

            #[doc(hidden)]
            struct #sel<#(#generic_fns),*>(#(#generic_fns),*);

            impl<#(#generic_fns,)* #(#generic_outs,)*> pco_pack::Consumer<#ty> for #sel<#(#generic_fns),*>
            where
                #(#where_clauses,)*
            {
                const FIELDS: &'static [&'static str] = &[#(#name_lits),*];
                #[doc(hidden)]
                const POPS_PAYLOAD: &'static [bool] = &[#(#pops),*];

                fn consume(
                    &mut self,
                    _cols: &mut <#ty as pco_pack::PcoPack>::Columns<'_>,
                    _row: usize,
                ) -> pco_pack::anyhow::Result<()> {
                    Err(pco_pack::anyhow::anyhow!(
                        "multi-variant fold! consumers dispatch through consume_variant"
                    ))
                }

                fn consume_variant(
                    &mut self,
                    __pco_cols: &mut <#ty as pco_pack::PcoPack>::Columns<'_>,
                    #row_ident: usize,
                    __pco_variant: usize,
                ) -> pco_pack::anyhow::Result<()> {
                    match __pco_variant {
                        #(#match_arms)*
                        other => Err(pco_pack::anyhow::anyhow!(
                            "multi-variant fold received unknown variant index {}",
                            other
                        )),
                    }
                }
            }

            #sel(#(#rewritten),*)
        }
    })
}
