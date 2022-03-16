mod utils;

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::quote;
use utils::{parse_arr_assign, parse_struct_assign, ArrayAssign, StructAssign};

/// Repeatedly assigns to a variable to increase the number of locations
/// while keeping the number of places constant.
#[proc_macro]
pub fn generate_locations(input: TokenStream) -> TokenStream {
  let ArrayAssign {
    var_name,
    var_val,
    num_locations,
  } = parse_arr_assign(input);

  let var_iter = std::iter::repeat(&var_name).take(num_locations);

  quote! {
    let mut #var_name = #var_val;
    #( #var_iter = #var_val; )*
  }
  .into()
}

/// Repeatedly borrows the same variable to create many places and many lifetimes.
#[proc_macro]
pub fn generate_unique_lifetimes(input: TokenStream) -> TokenStream {
  let ArrayAssign {
    var_name,
    var_val,
    num_locations,
  } = parse_arr_assign(input);

  let mut idents = vec![];
  for num in 1 ..= num_locations {
    idents.push(Ident::new(
      format!("borrow_{num}").as_str(),
      Span::call_site(),
    ));
  }

  quote! {
    let #var_name = #var_val;
    #( let #idents = &#var_name; )*
  }
  .into()
}

/// Assigns to a "main" variable and repeatedly creates temporary variables
/// which use the "main" variable as an input. Each temporary uses its value
/// to assign back to the "main" variable, generating infoflow between each temporary.
#[proc_macro]
pub fn generate_flow(input: TokenStream) -> TokenStream {
  let ArrayAssign {
    var_name,
    var_val,
    num_locations,
  } = parse_arr_assign(input);

  let mut idents = vec![];
  for num in 1 ..= num_locations {
    idents.push(Ident::new(
      format!("temp_{num}").as_str(),
      Span::call_site(),
    ));
  }

  quote! {
    let mut #var_name = #var_val;

    #(
      let #idents = #var_name + #var_val;
      #var_name += #idents;
    )*
  }
  .into()
}

/// Creates a struct with many fields, generating many places while
/// keeping the number of locations constant.
#[proc_macro]
pub fn generate_places(input: TokenStream) -> TokenStream {
  let StructAssign {
    var_name,
    struct_name,
    field_val,
    field_ty,
    num_fields,
  } = parse_struct_assign(input);

  let mut fields = vec![];
  for num in 1 ..= num_fields {
    fields.push(Ident::new(
      format!("field_{num}").as_str(),
      Span::call_site(),
    ));
  }

  quote! {
    struct #struct_name {
      #(#fields: #field_ty,)*
    }

    let #var_name = #struct_name {
      #(#fields: #field_val,)*
    };
  }
  .into()
}

/// Creates a struct with many fields, each having the type `&'a <type>`,
/// creating many places with one lifetime.
#[proc_macro]
pub fn generate_same_lifetime(input: TokenStream) -> TokenStream {
  let lt_ident = syn::Lifetime::new("'a", Span::call_site());

  let StructAssign {
    var_name,
    struct_name,
    field_val,
    field_ty,
    num_fields,
  } = parse_struct_assign(input);

  let mut fields = vec![];
  for num in 1 ..= num_fields {
    fields.push(Ident::new(
      format!("field_{num}").as_str(),
      Span::call_site(),
    ));
  }

  quote! {
    struct #struct_name<#lt_ident> {
      #(#fields: &#lt_ident #field_ty,)*
    }

    let #var_name = #struct_name {
      #(#fields: &#field_val,)*
    };
  }
  .into()
}
