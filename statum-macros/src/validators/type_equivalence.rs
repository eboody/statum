use quote::ToTokens;
use syn::{GenericArgument, PathArguments, ReturnType, Type};

pub(super) fn types_equivalent(left: &Type, right: &Type) -> bool {
    match (left, right) {
        (Type::Array(a), Type::Array(b)) => {
            types_equivalent(&a.elem, &b.elem) && expr_equivalent(&a.len, &b.len)
        }
        (Type::Group(a), Type::Group(b)) => types_equivalent(&a.elem, &b.elem),
        (Type::Infer(_), Type::Infer(_)) => true,
        (Type::Never(_), Type::Never(_)) => true,
        (Type::Paren(a), Type::Paren(b)) => types_equivalent(&a.elem, &b.elem),
        (Type::Path(a), Type::Path(b)) => {
            qself_equivalent(a.qself.as_ref(), b.qself.as_ref())
                && path_equivalent(&a.path, &b.path)
        }
        (Type::Ptr(a), Type::Ptr(b)) => {
            a.mutability.is_some() == b.mutability.is_some() && types_equivalent(&a.elem, &b.elem)
        }
        (Type::Reference(a), Type::Reference(b)) => {
            a.mutability.is_some() == b.mutability.is_some()
                && lifetime_equivalent(a.lifetime.as_ref(), b.lifetime.as_ref())
                && types_equivalent(&a.elem, &b.elem)
        }
        (Type::Slice(a), Type::Slice(b)) => types_equivalent(&a.elem, &b.elem),
        (Type::Tuple(a), Type::Tuple(b)) => {
            a.elems.len() == b.elems.len()
                && a.elems
                    .iter()
                    .zip(b.elems.iter())
                    .all(|(left_elem, right_elem)| types_equivalent(left_elem, right_elem))
        }
        _ => false,
    }
}

fn qself_equivalent(left: Option<&syn::QSelf>, right: Option<&syn::QSelf>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            left.position == right.position
                && types_equivalent(&left.ty, &right.ty)
                && left.as_token.is_some() == right.as_token.is_some()
        }
        (None, None) => true,
        _ => false,
    }
}

fn path_equivalent(left: &syn::Path, right: &syn::Path) -> bool {
    if left.leading_colon.is_some() != right.leading_colon.is_some() {
        return false;
    }
    if left.segments.len() != right.segments.len() {
        return false;
    }

    left.segments
        .iter()
        .zip(right.segments.iter())
        .all(|(left_segment, right_segment)| {
            left_segment.ident == right_segment.ident
                && path_arguments_equivalent(&left_segment.arguments, &right_segment.arguments)
        })
}

fn path_arguments_equivalent(left: &PathArguments, right: &PathArguments) -> bool {
    match (left, right) {
        (PathArguments::None, PathArguments::None) => true,
        (PathArguments::Parenthesized(left), PathArguments::Parenthesized(right)) => {
            left.inputs.len() == right.inputs.len()
                && left
                    .inputs
                    .iter()
                    .zip(right.inputs.iter())
                    .all(|(left_ty, right_ty)| types_equivalent(left_ty, right_ty))
                && match (&left.output, &right.output) {
                    (ReturnType::Default, ReturnType::Default) => true,
                    (ReturnType::Type(_, left_ty), ReturnType::Type(_, right_ty)) => {
                        types_equivalent(left_ty, right_ty)
                    }
                    _ => false,
                }
        }
        (PathArguments::AngleBracketed(left), PathArguments::AngleBracketed(right)) => {
            left.args.len() == right.args.len()
                && left
                    .args
                    .iter()
                    .zip(right.args.iter())
                    .all(|(left_arg, right_arg)| generic_argument_equivalent(left_arg, right_arg))
        }
        _ => false,
    }
}

fn generic_argument_equivalent(left: &GenericArgument, right: &GenericArgument) -> bool {
    match (left, right) {
        (GenericArgument::Lifetime(left), GenericArgument::Lifetime(right)) => left == right,
        (GenericArgument::Type(left), GenericArgument::Type(right)) => types_equivalent(left, right),
        (GenericArgument::Const(left), GenericArgument::Const(right)) => expr_equivalent(left, right),
        (GenericArgument::AssocType(left), GenericArgument::AssocType(right)) => {
            left.ident == right.ident
                && optional_angle_generics_equivalent(&left.generics, &right.generics)
                && types_equivalent(&left.ty, &right.ty)
        }
        (GenericArgument::AssocConst(left), GenericArgument::AssocConst(right)) => {
            left.ident == right.ident
                && optional_angle_generics_equivalent(&left.generics, &right.generics)
                && expr_equivalent(&left.value, &right.value)
        }
        (GenericArgument::Constraint(left), GenericArgument::Constraint(right)) => {
            left.ident == right.ident
                && optional_angle_generics_equivalent(&left.generics, &right.generics)
                && left.bounds.len() == right.bounds.len()
                && left
                    .bounds
                    .iter()
                    .zip(right.bounds.iter())
                    .all(|(left_bound, right_bound)| token_text(left_bound) == token_text(right_bound))
        }
        _ => false,
    }
}

fn expr_equivalent(left: &syn::Expr, right: &syn::Expr) -> bool {
    match (left, right) {
        (syn::Expr::Array(left), syn::Expr::Array(right)) => {
            left.elems.len() == right.elems.len()
                && left
                    .elems
                    .iter()
                    .zip(right.elems.iter())
                    .all(|(left_elem, right_elem)| expr_equivalent(left_elem, right_elem))
        }
        (syn::Expr::Binary(left), syn::Expr::Binary(right)) => {
            binary_op_equivalent(&left.op, &right.op)
                && expr_equivalent(&left.left, &right.left)
                && expr_equivalent(&left.right, &right.right)
        }
        (syn::Expr::Group(left), syn::Expr::Group(right)) => {
            expr_equivalent(&left.expr, &right.expr)
        }
        (syn::Expr::Lit(left), syn::Expr::Lit(right)) => lit_equivalent(&left.lit, &right.lit),
        (syn::Expr::Paren(left), syn::Expr::Paren(right)) => {
            expr_equivalent(&left.expr, &right.expr)
        }
        (syn::Expr::Path(left), syn::Expr::Path(right)) => {
            qself_equivalent(left.qself.as_ref(), right.qself.as_ref())
                && path_equivalent(&left.path, &right.path)
        }
        (syn::Expr::Reference(left), syn::Expr::Reference(right)) => {
            left.mutability.is_some() == right.mutability.is_some()
                && expr_equivalent(&left.expr, &right.expr)
        }
        (syn::Expr::Unary(left), syn::Expr::Unary(right)) => {
            unary_op_equivalent(&left.op, &right.op) && expr_equivalent(&left.expr, &right.expr)
        }
        _ => token_text(left) == token_text(right),
    }
}

fn lit_equivalent(left: &syn::Lit, right: &syn::Lit) -> bool {
    token_text(left) == token_text(right)
}

fn unary_op_equivalent(left: &syn::UnOp, right: &syn::UnOp) -> bool {
    std::mem::discriminant(left) == std::mem::discriminant(right)
}

fn binary_op_equivalent(left: &syn::BinOp, right: &syn::BinOp) -> bool {
    std::mem::discriminant(left) == std::mem::discriminant(right)
}

fn lifetime_equivalent(left: Option<&syn::Lifetime>, right: Option<&syn::Lifetime>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left == right,
        (None, None) => true,
        _ => false,
    }
}

fn optional_angle_generics_equivalent(
    left: &Option<syn::AngleBracketedGenericArguments>,
    right: &Option<syn::AngleBracketedGenericArguments>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            left.args.len() == right.args.len()
                && left
                    .args
                    .iter()
                    .zip(right.args.iter())
                    .all(|(left_arg, right_arg)| generic_argument_equivalent(left_arg, right_arg))
        }
        (None, None) => true,
        _ => false,
    }
}

fn token_text<T: ToTokens>(value: &T) -> String {
    value.to_token_stream().to_string()
}
