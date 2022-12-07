use hir::{HirId, LoopSource};
use rustc_hir::{
  self as hir,
  intravisit::{self, Visitor as HirVisitor},
  ExprKind, MatchSource, Node,
};
use rustc_span::{BytePos, Span};

use super::Spanner;
use crate::mir::utils::SpanExt;

// Collect all the spans for children beneath the visited node.
// For example, when visiting "if true { 1 } else { 2 }" then we
// should collect: "true" "1" "2"
struct ChildExprSpans {
  spans: Vec<Span>,
  item_span: Span,
}
impl<'hir> HirVisitor<'hir> for ChildExprSpans {
  fn visit_expr(&mut self, ex: &hir::Expr) {
    match ex.kind {
      // Don't take the span for the whole block, since we want to leave
      // curly braces to be associated with the outer statement
      ExprKind::Block(..) => {
        intravisit::walk_expr(self, ex);
      }
      // ForLoopDesgar case:
      //   The HIR span for a for-loop desugared to a match is *smaller*
      //   than the span of its children. So we have to explicitly recurse
      //   into the match arm instead of just taking the span for the match.
      //   See `forloop_some_relevant` for where this matters.
      //
      // Normal case:
      //   The SwitchInts for a normal match exclusively source-map to the patterns
      //   in the arms, not the matched expression. So to make sure that `match e { .. }`
      //   includes `e` when `match` is relevant, we exclude `e` from the child spans.
      ExprKind::Match(_, arms, MatchSource::ForLoopDesugar | MatchSource::Normal) => {
        for arm in arms {
          intravisit::walk_arm(self, arm);
        }
      }
      _ => {
        if let Some(span) = ex.span.as_local(self.item_span) {
          self.spans.push(span);
        }
      }
    }
  }

  fn visit_arm(&mut self, arm: &hir::Arm) {
    // We want the arm condition to be included in the outer span for the match,
    // so we only visit the arm body here.
    self.visit_expr(arm.body);
  }

  fn visit_stmt(&mut self, stmt: &hir::Stmt) {
    if let Some(span) = stmt.span.as_local(self.item_span) {
      self.spans.push(span);
    }
  }
}

/// Which parts of a HIR node's span should be included for a matching MIR node
#[derive(Clone, Copy)]
pub enum EnclosingHirSpans {
  /// The entire span
  Full,

  /// The spans of the node minus its children
  OuterOnly,

  /// No span
  None,
}

macro_rules! try_span {
  ($self:expr, $span:expr) => {
    match $span.as_local($self.item_span) {
      Some(span) if !$self.invalid_span(span) => span,
      _ => {
        return None;
      }
    }
  };
}

impl<'tcx> Spanner<'tcx> {
  pub fn hir_spans(&self, id: HirId, mode: EnclosingHirSpans) -> Option<Vec<Span>> {
    let hir = self.tcx.hir();
    let span = try_span!(self, hir.span(id));
    let inner_spans = match hir.get(id) {
      Node::Expr(expr) => match expr.kind {
        ExprKind::Loop(_, _, loop_source, header) => match loop_source {
          LoopSource::ForLoop | LoopSource::While => {
            vec![expr.span.trim_start(header).unwrap_or(expr.span)]
          }

          LoopSource::Loop => vec![expr.span.with_lo(expr.span.lo() + BytePos(4))],
        },
        ExprKind::Break(..) => return None,
        _ => {
          let mut visitor = ChildExprSpans {
            spans: Vec::new(),
            item_span: self.item_span,
          };
          intravisit::walk_expr(&mut visitor, expr);

          visitor.spans
        }
      },
      Node::Stmt(stmt) => {
        let mut visitor = ChildExprSpans {
          spans: Vec::new(),
          item_span: self.item_span,
        };
        intravisit::walk_stmt(&mut visitor, stmt);
        visitor.spans
      }
      Node::Param(_param) => vec![],
      _ => {
        return None;
      }
    };

    Some(match mode {
      EnclosingHirSpans::Full => vec![span],
      EnclosingHirSpans::OuterOnly => span.subtract(inner_spans),
      EnclosingHirSpans::None => vec![],
    })
  }
}
