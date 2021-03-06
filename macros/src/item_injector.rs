use crate::header_builder::FnHeaderBuilder;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{
    Attribute, Block, FnArg, Ident, ImplItem, ImplItemMethod, Item, ItemFn, ItemImpl, ItemMod,
    ItemTrait, Pat, PatIdent, PatType, Signature, TraitItem, TraitItemMethod,
};

pub fn inject_item(item: &mut Item) {
    match *item {
        Item::Fn(ref mut item_fn) => inject_fn(item_fn),
        Item::Mod(ref mut item_mod) => inject_mod(item_mod),
        Item::Trait(ref mut item_trait) => inject_trait(item_trait),
        Item::Impl(ref mut item_impl) => inject_impl(item_impl),
        _ => (),
    }
}

fn inject_fn(item_fn: &mut ItemFn) {
    inject_any_fn(
        &FnHeaderBuilder::StaticFn,
        &item_fn.attrs,
        &mut item_fn.sig,
        &mut *item_fn.block,
    );
}

fn inject_mod(item_mod: &mut ItemMod) {
    if is_not_mockable(&item_mod.attrs) {
        return;
    }
    item_mod
        .content
        .iter_mut()
        .flat_map(|c| &mut c.1)
        .for_each(inject_item)
}

fn inject_trait(item_trait: &mut ItemTrait) {
    if is_not_mockable(&item_trait.attrs) {
        return;
    }
    for item in &mut item_trait.items {
        if let TraitItem::Method(TraitItemMethod {
            ref attrs,
            ref mut sig,
            default: Some(ref mut block),
            ..
        }) = *item
        {
            inject_any_fn(&FnHeaderBuilder::TraitDefault, attrs, sig, block);
        }
    }
}

fn inject_impl(item_impl: &mut ItemImpl) {
    if is_not_mockable(&item_impl.attrs) {
        return;
    }
    let builder = match item_impl.trait_ {
        Some((_, ref path, _)) => FnHeaderBuilder::TraitImpl(&path.segments),
        None => FnHeaderBuilder::StructImpl,
    };
    for impl_item in &mut item_impl.items {
        if let ImplItem::Method(ref mut item_method) = *impl_item {
            if is_impl_fn_mockabile(&builder, item_method) {
                inject_any_fn(
                    &builder,
                    &item_method.attrs,
                    &mut item_method.sig,
                    &mut item_method.block,
                );
            }
        }
    }
}

fn is_impl_fn_mockabile(builder: &FnHeaderBuilder, item_method: &ImplItemMethod) -> bool {
    if let FnHeaderBuilder::TraitImpl(ref segments) = *builder {
        if let Some(segment) = segments.last() {
            if segment.arguments.is_empty() && segment.ident == "Drop" {
                if item_method.sig.ident == "drop" {
                    return false;
                }
            }
        }
    }
    true
}

fn inject_any_fn(
    builder: &FnHeaderBuilder,
    attrs: &Vec<Attribute>,
    fn_decl: &mut Signature,
    block: &mut Block,
) {
    if fn_decl.constness.is_some()
        || fn_decl.unsafety.is_some()
        || fn_decl.variadic.is_some()
        || is_not_mockable(attrs)
    {
        return;
    }
    unignore_fn_args(&mut fn_decl.inputs);
    let header_stmt = builder.build(fn_decl, block.brace_token.span);
    block.stmts.insert(0, header_stmt);
}

fn unignore_fn_args(inputs: &mut Punctuated<FnArg, Comma>) {
    for (i, fn_arg) in inputs.iter_mut().enumerate() {
        if let FnArg::Typed(PatType { ref mut pat, .. }) = *fn_arg {
            let (span, attrs) = match **pat {
                Pat::Wild(ref pat_wild) => {
                    (pat_wild.underscore_token.spans[0], pat_wild.attrs.clone())
                }
                _ => continue,
            };
            *pat = Box::new(Pat::Ident(PatIdent {
                by_ref: None,
                mutability: None,
                ident: Ident::new(&format!("__mocktopus_unignored_argument_{}__", i), span),
                subpat: None,
                attrs,
            }));
        }
    }
}

const INJECTOR_STOPPER_ATTRS: [&str; 2] = ["mockable", "not_mockable"];

fn is_not_mockable(attrs: &Vec<Attribute>) -> bool {
    attrs
        .iter()
        .filter_map(|a| a.path.segments.last())
        .map(|segment| segment.ident.to_string())
        .any(|i| INJECTOR_STOPPER_ATTRS.contains(&&*i))
}
