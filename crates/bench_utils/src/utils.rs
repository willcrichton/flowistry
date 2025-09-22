use std::str::FromStr;

use if_chain::if_chain;
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::{ToTokens, quote};

pub fn parse_expr<T: FromStr>(expr: syn::Expr) -> T {
  expr.to_token_stream().to_string().parse().ok().unwrap()
}

pub struct ArrayAssign {
  pub var_name: syn::Expr,
  pub var_val: syn::Expr,
  pub num_locations: usize,
}

/// Parses the expression:
/// `var_name: [_; num_locations] = var_val`
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
/// `var_name: struct_name<[field_ty; num_fields]> = field_val`
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

/// Represents a "level" of the nested struct
#[derive(Clone)]
pub struct TreeLevel {
  ident: Ident,
  pub def: TokenStream2,
  instance_ident: Ident,
  pub instantiation: TokenStream2,
}

impl TreeLevel {
  pub fn new(
    level: usize,
    fields: Vec<Ident>,
    child_level: Option<&TreeLevel>,
    field_val: &syn::Expr,
    field_ty: &syn::Type,
  ) -> TreeLevel {
    let ident = Ident::new(&format!("struct_{level}"), Span::call_site());
    let instance_ident = Ident::new(&format!("struct_{level}_inst"), Span::call_site());
    let type_ident =
      Ident::new(&field_ty.to_token_stream().to_string(), Span::call_site());

    // If a child level exists, fields of the current level should
    // have the type of the child struct (if not, fall back to primitive)
    let field_ty = if let Some(child) = child_level {
      &child.ident
    } else {
      &type_ident
    };

    let def = quote! {
      #[derive(Clone)]
      struct #ident {
        #(#fields: #field_ty,)*
      }
    };

    let instantiation = match child_level {
      Some(child) => {
        let field_val = &child.instance_ident;

        quote! {
          let #instance_ident = #ident {
            #(#fields: #field_val.clone(),)*
          };
        }
      }
      None => {
        quote! {
          let #instance_ident = #ident {
            #(#fields: #field_val,)*
          };
        }
      }
    };

    TreeLevel {
      ident,
      def,
      instance_ident,
      instantiation,
    }
  }
}
