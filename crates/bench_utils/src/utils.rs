use std::str::FromStr;

use if_chain::if_chain;
use proc_macro::TokenStream;
use quote::ToTokens;

pub fn parse_expr<T: FromStr>(expr: syn::Expr) -> T {
  expr.to_token_stream().to_string().parse().ok().unwrap()
}

pub struct ArrayAssign {
  pub var_name: syn::Expr,
  pub var_val: syn::Expr,
  pub num_locations: usize,
}

/// Parses the expression:
/// <pre>
/// <b>var_name</b>: [_; <b>num_locations</b>] = <b>var_val</b>
/// </pre>
pub fn parse_arr_assign(input: TokenStream) -> ArrayAssign {
  let expr: syn::ExprAssign = syn::parse(input).unwrap();

  if_chain! {
    if let syn::Expr::Type(expr_ty) = *expr.left;
    if let syn::Type::Array(arr_ty) = *expr_ty.ty;
    then {
      return ArrayAssign {
          var_name: *expr_ty.expr,
          var_val: *expr.right,
          num_locations: parse_expr(arr_ty.len),
      };
    }
  }

  panic!();
}

pub struct StructAssign {
  pub var_name: syn::Expr,
  pub struct_name: proc_macro2::Ident,
  pub field_val: syn::Expr,
  pub field_ty: syn::Type,
  pub num_fields: usize,
}

/// Parses the expression:
/// <pre>
/// <b>var_name</b>: <b>struct_name</b><[<b>field_ty</b>; <b>num_fields</b>]> = <b>field_val</b>
/// </pre>
pub fn parse_struct_assign(input: TokenStream) -> StructAssign {
  let expr: syn::ExprAssign = syn::parse(input).unwrap();

  if_chain! {
    if let syn::Expr::Type(expr_ty) = *expr.left;

    if let syn::Type::Path(struct_ty) = *expr_ty.ty;
    let ty_segments = struct_ty.path.segments.first().unwrap();

    if let syn::PathArguments::AngleBracketed(generics) = &ty_segments.arguments;
    let arr_generic = generics.args.first().unwrap();

    if let syn::GenericArgument::Type(syn::Type::Array(arr_ty)) = arr_generic;

    then {
      return StructAssign {
        var_name: *expr_ty.expr,
        struct_name: ty_segments.ident.clone(),
        field_ty: *arr_ty.elem.clone(),
        field_val: *expr.right,
        num_fields: parse_expr(arr_ty.len.clone()),
      };
    }
  }

  panic!();
}
