use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use std::vec::Vec;
use syn::Ident;

#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    let struct_name = input.ident;
    let builder_name = Ident::new(&format!("{}Builder", struct_name), Span::call_site());

    match input.data {
        syn::Data::Struct(data) => {
            let mut builder_fields = Vec::new();
            let mut setter_methods = Vec::new();
            let mut build_assignments = Vec::new();
            let mut build_inits = Vec::new();

            for field in data.fields.iter() {
                let name = &field.ident;
                let ty = &field.ty;
                let mut each_name: Option<String> = None;

                for attr in &field.attrs {
                    if attr.path().is_ident("builder") {
                        if let Err(err) = attr.parse_nested_meta(|meta| {
                            if meta.path.is_ident("each") {
                                let value = meta.value()?;
                                let lit: syn::LitStr = value.parse()?;
                                each_name = Some(lit.value());
                                Ok(())
                            } else {
                                // Err(meta.error("expected `builder(each = \"...\")`"))
                                Err(syn::Error::new_spanned(
                                    &attr.meta,
                                    "expected `builder(each = \"...\")`",
                                ))
                            }
                        }) {
                            return err.to_compile_error().into();
                        }
                    }
                }

                if let Some(each_name) = each_name {
                    let each_ident = Ident::new(&each_name, Span::call_site());
                    let inner_type = extract_option_inner_type(ty, "Vec").unwrap();

                    builder_fields.push(quote! { #name: std::vec::Vec<#inner_type> });
                    setter_methods.push(quote! {
                        fn #each_ident(&mut self, #each_ident: #inner_type) -> &mut Self {
                            self.#name.push(#each_ident);
                            self
                        }
                    });
                    build_assignments.push(quote! {
                        #name: self.#name.clone()
                    });
                    build_inits.push(quote! {
                        #name: std::vec::Vec::new()
                    })
                } else if let Some(inner_type) = extract_option_inner_type(ty, "Option") {
                    builder_fields.push(quote! { #name: std::option::Option<#inner_type> });
                    setter_methods.push(quote! {
                        fn #name(&mut self, #name: #inner_type) -> &mut Self {
                            self.#name = std::option::Option::Some(#name);
                            self
                        }
                    });
                    build_assignments.push(quote! {
                        #name: self.#name.take()
                    });
                    build_inits.push(quote! {
                        #name: std::option::Option::None
                    });
                } else {
                    builder_fields.push(quote! { #name: std::option::Option<#ty> });
                    setter_methods.push(quote! {
                        fn #name(&mut self, #name: #ty) -> &mut Self {
                            self.#name = std::option::Option::Some(#name);
                            self
                        }
                    });
                    build_assignments.push(quote! {
                        #name: self.#name.take().ok_or(format!("{} is not set", stringify!(#name)))?
                    });
                    build_inits.push(quote! {
                        #name: std::option::Option::None
                    });
                }
            }

            quote! {
               struct #builder_name {
                   #(#builder_fields),*
               }

               impl #builder_name {
                    #(#setter_methods)*

                    fn build(&mut self) -> std::result::Result<#struct_name, std::boxed::Box<dyn std::error::Error>> {
                        std::result::Result::Ok(#struct_name {
                            #(#build_assignments),*
                        })
                    }
               }

               impl #struct_name {
                   pub fn builder() -> #builder_name {
                       #builder_name {
                           #(#build_inits),*
                       }
                   }
               }
            }
            .into()
        }
        _ => unimplemented!(),
    }
}

fn extract_option_inner_type<'a>(ty: &'a syn::Type, type_name: &str) -> Option<&'a syn::Type> {
    match ty {
        syn::Type::Path(type_path) => {
            let segment = type_path.path.segments.last()?;
            if segment.ident != type_name {
                return None;
            }
            match &segment.arguments {
                syn::PathArguments::AngleBracketed(args) => match args.args.first() {
                    Some(syn::GenericArgument::Type(inner_type)) => Some(inner_type),
                    _ => None,
                },
                _ => None,
            }
        }
        _ => None,
    }
}
