use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Expr, Lit, Meta};

fn get_rename_value(attrs: &[Attribute]) -> Option<String> {
    attrs
        .iter()
        .filter(|&attr| attr.path().is_ident("rename_variant"))
        .find_map(|attr| {
            if let Meta::NameValue(x) = &attr.meta {
                if let Expr::Lit(ref y) = x.value {
                    if let Lit::Str(ref z) = y.lit {
                        return Some(z.value());
                    }
                }
            }
            None
        })
}

#[proc_macro_derive(EnumToArrayStr, attributes(rename_variant))]
pub fn enum_variants_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let variants = match input.data {
        Data::Enum(data_enum) => data_enum.variants,
        _ => panic!("EnumVariants can only be derived for enums"),
    };

    let variant_count = variants.len();
    let variant_names: Vec<_> = variants
        .iter()
        .map(|v| get_rename_value(&v.attrs).unwrap_or_else(|| v.ident.to_string()))
        .collect();

    let expanded = quote! {
        impl #name {
            pub fn to_array_str() -> [&'static str; #variant_count] {
                [#(#variant_names),*]
            }
        }
    };

    TokenStream::from(expanded)
}
