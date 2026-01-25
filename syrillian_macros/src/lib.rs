use proc_macro::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{Attribute, Data, Error, Field, Fields, Meta};

#[proc_macro_derive(UniformIndex)]
pub fn uniform_index(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::ItemEnum);

    if input.variants.is_empty() {
        return Error::new(
            input.span(),
            "Uniform Shader Indexers must have at least one variant",
        )
        .to_compile_error()
        .into();
    }

    let type_ident = &input.ident;

    let type_ident_str = type_ident
        .to_string()
        .replace("Uniform", "")
        .replace("Index", "");

    let variants = input.variants.iter().map(|var| &var.ident);
    let variants2 = input.variants.iter().map(|var| &var.ident);
    let index_max = input.variants.len() - 1;
    let amount_addon_impl = match input.variants.len() {
        0 => quote! { impl ::syrillian_utils::ShaderUniformSingleIndex for #type_ident {} },
        _ => quote! { impl ::syrillian_utils::ShaderUniformMultiIndex for #type_ident {} },
    };

    quote! {
        impl ::syrillian_utils::ShaderUniformIndex for #type_ident {
            const MAX: usize = #index_max;

            #[inline]
            fn index(&self) -> usize {
               *self as usize
            }

            #[inline]
            fn by_index(index: usize) -> Option<Self> {
                index.try_into().ok()
            }

            #[inline]
            fn name() -> &'static str {
                #type_ident_str
            }
        }

        #amount_addon_impl

        impl ::std::convert::TryFrom<usize> for #type_ident {
            type Error = ();
            fn try_from(value: usize) -> Result<Self, Self::Error> {
                match value {
                    #(x if x == Self::#variants as usize => Ok(Self::#variants),)*
                    _ => Err(()),
                }
            }
        }

        impl ::std::convert::TryFrom<u64> for #type_ident {
            type Error = ();
            fn try_from(value: u64) -> Result<Self, Self::Error> {
                match value {
                    #(x if x == Self::#variants2 as u64 => Ok(Self::#variants2),)*
                    _ => Err(()),
                }
            }
        }
    }
    .into()
}

/// This will start a preconfigured runtime for your App. Make sure you have a Default implementation
#[proc_macro_derive(SyrillianApp)]
pub fn syrillian_app(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    let logger = cfg!(feature = "derive_tracing_subscriber").then(|| {
        quote!(
            ::tracing_subscriber::FmtSubscriber::builder()
                .with_env_filter(::tracing_subscriber::EnvFilter::from_default_env())
                .init();
        )
    });

    let app_name = &input.ident;

    quote! {
        fn main() {
            let app = <#app_name as ::syrillian::AppRuntime>::configure(stringify!(#app_name), 800, 600);

            #logger

            if let Err(e) = ::syrillian::AppSettings::run(app) {
                ::syrillian::tracing::error!("{e}");
            }
        }
    }.into()
}

#[proc_macro_derive(Reflect, attributes(reflect, reflect_all, dont_reflect))]
pub fn reflect_derive(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    if !input.generics.params.is_empty() {
        return Error::new(
            input.generics.span(),
            "Reflect cannot be derived for generic components",
        )
            .to_compile_error()
            .into();
    }

    let reflect_all = has_attr(&input.attrs, "reflect_all");

    let type_ident = &input.ident;

    let Data::Struct(input) = input.data else {
        return Error::new(
            input.ident.span(),
            "Reflect cannot be derived for non-named-struct components",
        )
            .to_compile_error()
            .into();
    };

    let mut reflected = Vec::new();

    if let Fields::Named(fields) = input.fields {
        for field in &fields.named {
            if !should_reflect(field, reflect_all) {
                continue;
            }

            let field_ident = field.ident.as_ref().expect("Named fields have names");
            let field_type = &field.ty;

            reflected.push(quote! {
                ::syrillian::core::reflection::ReflectedField {
                    name: stringify!( #field_ident ),
                    offset: std::mem::offset_of!( #type_ident, #field_ident ),
                    type_id: std::any::TypeId::of::<#field_type>(),
                }
            });
        }
    };

    let registration = quote! {
        ::syrillian::inventory::submit! {
            <#type_ident as ::syrillian::core::reflection::PartialReflect>::DATA
        }
    };

    let reflect_impl = quote! {
        impl ::syrillian::core::reflection::PartialReflect for #type_ident {
            const DATA: ::syrillian::core::reflection::ReflectedTypeInfo = ::syrillian::core::reflection::ReflectedTypeInfo {
                type_id: std::any::TypeId::of::<#type_ident>(),
                type_name: concat!(module_path!(), "::", stringify!(#type_ident)),
                short_name: stringify!(#type_ident),
                fields: &[#( #reflected ),*],
            };
        }
    };

    quote! {
        #reflect_impl
        #registration
    }
        .into()
}

fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    for attr in attrs {
        if let Meta::Path(path) = &attr.meta
            && path.segments.iter().any(|s| s.ident == name)
        {
            return true;
        }
    }
    false
}

fn should_reflect(field: &Field, reflect_all: bool) -> bool {
    if reflect_all && !has_attr(&field.attrs, "dont_reflect") {
        return true;
    }

    has_attr(&field.attrs, "reflect")
}

#[proc_macro_attribute]
pub fn reflect_fn(attr: TokenStream, input: TokenStream) -> TokenStream {
    let _ = syn::parse_macro_input!(attr as syn::parse::Nothing);
    let func = syn::parse_macro_input!(input as syn::ItemFn);
    let sig = &func.sig;
    let ident = &func.sig.ident;

    quote! {
        #func

        ::syrillian::inventory::submit! {
            ::syrillian::reflection::FunctionInfo {
                name: stringify!(#ident),
                module_path: module_path!(),
                full_name: concat!(module_path!(), "::", stringify!(#ident)),
                signature: stringify!(#sig),
            }
        }
    }
        .into()
}

// TODO: macro-ize some things related to proxy data / scene proxies and in general
// #[proc_macro_attribute]
// fn proxy_data_fn(_: TokenStream, input: TokenStream) -> TokenStream {
//     let func = syn::parse_macro_input!(input as ImplItemFn);
//     match &func.sig.output {
//         ReturnType::Default => wrap_update_render(func),
//         ReturnType::Type(..) => wrap_setup_render(func),
//     }.into()
// }
//
// fn wrap_setup_render(mut func: ImplItemFn) -> proc_macro2::TokenStream {
//     let output = &func.sig.output;
//     let ty = match output {
//         ReturnType::Default => unreachable!(),
//         ReturnType::Type(_, ty) => ty,
//     }.clone();
//
//     let new_ident = Ident::new("__inner_setup_render", func.sig.ident.span());
//     func.sig.ident = new_ident.clone();
//
//     quote! {
//         fn setup_render(&mut self, renderer: &Renderer, local_to_world: &Matrix4<f32>) -> Box<dyn Any> {
//             #func
//             let proxy_data = #new_ident(self, renderer, data, window, local_to_world);
//             Box::new(proxy_data)
//         }
//     }
// }
//
// fn wrap_update_render(mut func: ImplItemFn) -> proc_macro2::TokenStream {
//     if func.sig.inputs.len() < 3 {
//         return Error::new_spanned(
//             &func.sig.ident,
//             "expected at least 3 arguments"
//         ).to_compile_error();
//     }
//
//     let third = func.sig.inputs[2].clone();
//
//     let ty = match &third {
//         FnArg::Typed(pat_type) => &*pat_type.ty,
//         FnArg::Receiver(_) => {
//             return Error::new_spanned(
//                 third,
//                 "expected a typed argument, but found `self`"
//             ).to_compile_error().into();
//         }
//     };
//
//     let new_ident = Ident::new("__inner_update_render", func.sig.ident.span());
//     func.sig.ident = new_ident.clone();
//
//     quote! {
//         fn update_render(&mut self, renderer: &Renderer, data: &mut dyn Any, window: &Window, local_to_world: &Matrix4<f32>) {
//             #func
//             let data: #ty = proxy_data_mut!(data);
//
//             #new_ident(self, renderer, data, window, local_to_world)
//         }
//     }
// }
