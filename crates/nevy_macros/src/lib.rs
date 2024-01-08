use macro_state::{proc_append_state, proc_read_state_vec};
use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{parse_macro_input, DeriveInput};


#[proc_macro_derive(NetMessage)]
pub fn message(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    proc_append_state("registered_messages", &name.to_string()).expect("Failed to append state!");

    let expanded = quote! {
        impl NetMessage for #name {}
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(NetMessageRegistry)]
pub fn message_tags(_attr: TokenStream) -> TokenStream {    
    proc_append_state("registered_messages", "MyMessage").expect("Failed to append state!");

    let registered_messages = proc_read_state_vec("registered_messages");

    let mut impls = Vec::new();
    for (idx, name) in registered_messages.iter().enumerate() {
        let msg_name_ident = format_ident!("{}", name);

        impls.push(quote! {
            impl NetMessageId for #msg_name_ident {
                fn id() -> u32 {
                    #idx as u32
                }
            }
        });
    }

    let expanded = quote! {
        #(#impls)*
    };

    TokenStream::from(expanded)
}
