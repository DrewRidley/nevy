extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(NetBundle, attributes(client, server, sync, marker, init))]
pub fn net_bundle_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let shorthand_name = name.to_string().replace("Bundle", "");

    let plugin_server_name = format!("NetServer{}Plugin", shorthand_name);
    let plugin_server_ident = syn::Ident::new(&plugin_server_name, name.span());

    let plugin_client_name = format!("NetClient{}Plugin", shorthand_name);
    let plugin_client_ident = syn::Ident::new(&plugin_client_name, name.span());

    let server_bundle_name = format!("Server{}", name);
    let server_bundle_ident = syn::Ident::new(&server_bundle_name, name.span());

    //Find any field that is zero-sized or empty.
    //This will be the marker component used to identify this specific bundle.



    // Extract fields, filtering out those with #[client] attribute
    let fields = if let syn::Data::Struct(data) = &input.data {
        data.fields.iter().filter_map(|f| {
            // Check for #[client] attribute
            let is_client_field = f.attrs.iter().any(|attr| attr.path.is_ident("client"));
            if is_client_field {
                None
            } else {
                // Create a new field without the #[client] attribute
                let field_vis = &f.vis;
                let field_ty = &f.ty;
                let field_ident = &f.ident;
                let new_field = quote! { #field_vis #field_ident: #field_ty };
                Some(new_field)
            }
        }).collect::<Vec<_>>()
    } else {
        Vec::new()  // Handle other cases or provide an error
    };

    let expanded = quote! {
        pub struct #plugin_client_ident;

        impl Plugin for #plugin_client_ident {
            fn build(&self, app: &mut bevy::prelude::App) {
                // Client plugin build logic
            }
        }

        impl Default for #plugin_client_ident {
            fn default() -> Self {
                Self
            }
        }

        pub struct #plugin_server_ident;

        impl Plugin for #plugin_server_ident {
            fn build(&self, app: &mut bevy::prelude::App) {
                // Server plugin build logic
            }
        }

        impl Default for #plugin_server_ident {
            fn default() -> Self {
                Self
            }
        }

        #[derive(Bundle)]
        pub struct #server_bundle_ident {
            #( #fields ),*
        }

        impl NetBundle for #name {
            type ClientPlugin = #plugin_client_ident;
            type ServerPlugin = #plugin_server_ident;
            type ServerBundle = #server_bundle_ident;
        }
    };

    TokenStream::from(expanded)
}