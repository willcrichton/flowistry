//! Macros used to generate programs for Flowistry's benchmarks
mod utils;

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::quote;
use utils::{
  ArrayAssign, StructAssign, TreeLevel, parse_arr_assign, parse_struct_assign,
};

/// Repeatedly assigns to a variable to increase the number of locations
/// while keeping the number of places constant. For example:
/// ```rust
/// use bench_utils::generate_locations;
/// generate_locations!(foo: [i32; 10] = 1);
/// ```
/// generates a program which assigns `foo = 1` 10 times.
#[proc_macro]
pub fn generate_locations(input: TokenStream) -> TokenStream {
  let ArrayAssign {
    var_name,
    var_val,
    num_locations,
  } = parse_arr_assign(input);

  let var_iter = std::iter::repeat_n(&var_name, num_locations);

  quote! {
    let mut #var_name = #var_val;
    #( #var_iter = #var_val; )*
  }
  .into()
}

/// Repeatedly borrows the same variable to create many places, locations, and lifetimes.
/// For example:
/// ```rust
/// use bench_utils::generate_unique_lifetimes;
/// generate_unique_lifetimes!(foo: [i32; 10] = 1);
/// ```
/// generates a program which creates 10 "borrow" variables, each assigned to `&foo`:
/// ```rust
/// let foo = 1;
/// let borrow_1 = &foo;
/// let borrow_2 = &foo;
/// // ...
/// ```
#[proc_macro]
pub fn generate_unique_lifetimes(input: TokenStream) -> TokenStream {
  let ArrayAssign {
    var_name,
    var_val,
    num_locations,
  } = parse_arr_assign(input);

  let idents = (1 ..= num_locations)
    .map(|num| Ident::new(format!("borrow_{num}").as_str(), Span::call_site()));

  quote! {
    let #var_name = #var_val;
    #( let #idents = &#var_name; )*
  }
  .into()
}

/// Assigns to a "main" variable and repeatedly creates temporary variables
/// which use the "main" variable as an input. Each temporary uses its value
/// to assign back to the "main" variable, generating infoflow between each temporary.
/// For example:
/// ```rust
/// use bench_utils::generate_flow;
/// generate_flow!(foo: [i32; 10] = 1);
/// ```
/// generates
/// ```rust
/// let mut foo = 1;
/// let temp_1 = foo;
/// foo = temp_1;
/// let temp_2 = foo;
/// foo = temp_2;
/// // ...
/// ```
#[proc_macro]
pub fn generate_flow(input: TokenStream) -> TokenStream {
  let ArrayAssign {
    var_name,
    var_val,
    num_locations,
  } = parse_arr_assign(input);

  let idents = (1 ..= num_locations)
    .map(|num| Ident::new(format!("temp_{num}").as_str(), Span::call_site()));

  quote! {
    let mut #var_name = #var_val;

    #(
      let #idents = #var_name;
      #var_name = #idents;
    )*
  }
  .into()
}

/// Creates a struct with many fields, generating many places while
/// keeping the number of locations constant. For example:
/// ```rust
/// use bench_utils::generate_places;
/// generate_places!(foo: PlaceStruct<[i32; 3]> = 1);
/// ```
/// generates a struct called `PlaceStruct` with 3 `i32` fields and assigns
/// `foo` to an instantiation of the `PlaceStruct` where each field is `1`:
#[proc_macro]
pub fn generate_places(input: TokenStream) -> TokenStream {
  let StructAssign {
    var_name,
    struct_name,
    field_val,
    field_ty,
    num_fields,
  } = parse_struct_assign(input);

  let fields = (1 ..= num_fields)
    .map(|num| Ident::new(format!("field_{num}").as_str(), Span::call_site()))
    .collect::<Vec<_>>();

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
/// creating many places with one lifetime. For example:
/// ```rust
/// use bench_utils::generate_same_lifetime;
/// generate_same_lifetime!(foo: LifetimesStruct<[i32; 3]> = 1);
/// ```
/// generates a `LifetimesStruct<'a>` struct with 3 `&'a i32` fields and assigns
/// `foo` to an instantiation of the struct, with each field having the value `&1`.
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

  let fields = (1 ..= num_fields)
    .map(|num| Ident::new(format!("field_{num}").as_str(), Span::call_site()))
    .collect::<Vec<_>>();

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

/// Creates a struct with deeply-nested fields. For example:
/// ```rust
/// use bench_utils::generate_nested_struct;
/// generate_nested_struct!(foo: NestedStruct<[i32; 3]> = 1);
/// ```
/// generates 3 structs for each "level" of the nesting, with 3 fields each:
/// ```rust
/// struct struct_1 {
///   field_1: i32,
///   // ...
/// }
/// struct struct_2 {
///   field_1: struct_1,
///   // ...
/// }
/// struct struct_3 {
///   field_1: struct_2,
///   // ...
/// }
/// ```
/// the macro then instantiates each level of the tree, resulting in a final struct
/// with nⁿ `i32` fields.
#[proc_macro]
pub fn generate_nested_struct(input: TokenStream) -> TokenStream {
  let StructAssign {
    field_val,
    field_ty,
    num_fields,
    ..
  } = parse_struct_assign(input);

  let fields = (1 ..= num_fields)
    .map(|num| Ident::new(format!("field_{num}").as_str(), Span::call_site()))
    .collect::<Vec<_>>();

  // Create and instantiate structs for each "level" of the nested struct
  let mut levels = vec![];
  for level_num in (0 ..= num_fields).rev() {
    let level = TreeLevel::new(
      level_num,
      fields.clone(),
      levels.last(),
      &field_val,
      &field_ty,
    );
    levels.push(level);
  }

  let defs = levels.iter().map(|level| level.def.clone());
  let instants = levels.iter().map(|level| level.instantiation.clone());
  quote! {
    #(#defs)*

    #(#instants)*
  }
  .into()
}
