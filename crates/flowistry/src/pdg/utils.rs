use log::debug;
use rustc_hir::{def_id::DefId, Unsafety};
use rustc_middle::ty::{self, GenericArgsRef, Instance, ParamEnv, TyCtxt};
use rustc_span::ErrorGuaranteed;
use rustc_target::spec::abi::Abi;

/// This exists to distinguish different types of functions, which is necessary
/// because depending on the type of function, the method of requesting its
/// signature from `TyCtxt` differs.
///
/// In addition generators also return true for `TyCtxt::is_closure` but must
/// request their signature differently. Thus we factor that determination out
/// into this enum.
#[derive(Clone, Copy, Eq, PartialEq)]
enum FunctionKind {
  Closure,
  Generator,
  Plain,
}

impl FunctionKind {
  fn for_def_id(tcx: TyCtxt, def_id: DefId) -> Result<Self, ErrorGuaranteed> {
    if tcx.generator_kind(def_id).is_some() {
      Ok(Self::Generator)
    } else if tcx.is_closure(def_id) {
      Ok(Self::Closure)
    } else if tcx.def_kind(def_id).is_fn_like() {
      Ok(Self::Plain)
    } else {
      Err(
        tcx
          .sess
          .span_err(tcx.def_span(def_id), "Expected this item to be a function."),
      )
    }
  }
}

#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum FnResolution<'tcx> {
  Final(ty::Instance<'tcx>),
  Partial(DefId),
}

impl<'tcx> PartialOrd for FnResolution<'tcx> {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    use FnResolution::*;
    match (self, other) {
      (Final(_), Partial(_)) => Some(std::cmp::Ordering::Greater),
      (Partial(_), Final(_)) => Some(std::cmp::Ordering::Less),
      (Partial(slf), Partial(otr)) => slf.partial_cmp(otr),
      (Final(slf), Final(otr)) => match slf.def.partial_cmp(&otr.def) {
        Some(std::cmp::Ordering::Equal) => slf.args.partial_cmp(otr.args),
        result => result,
      },
    }
  }
}

impl<'tcx> Ord for FnResolution<'tcx> {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.partial_cmp(other).unwrap()
  }
}

impl<'tcx> FnResolution<'tcx> {
  pub fn def_id(self) -> DefId {
    match self {
      FnResolution::Final(f) => f.def_id(),
      FnResolution::Partial(p) => p,
    }
  }

  /// Get the most precise type signature we can for this function, erase any
  /// regions and discharge binders.
  ///
  /// Returns an error if it was impossible to get any signature.
  ///
  /// Emits warnings if a precise signature could not be obtained or there
  /// were type variables not instantiated.
  pub fn sig(self, tcx: TyCtxt<'tcx>) -> Result<ty::FnSig<'tcx>, ErrorGuaranteed> {
    let sess = tcx.sess;
    let def_id = self.def_id();
    let def_span = tcx.def_span(def_id);
    let fn_kind = FunctionKind::for_def_id(tcx, def_id)?;
    let late_bound_sig = match (self, fn_kind) {
      (FnResolution::Final(sub), FunctionKind::Generator) => {
        let gen = sub.args.as_generator();
        ty::Binder::dummy(ty::FnSig {
          inputs_and_output: tcx.mk_type_list(&[gen.resume_ty(), gen.return_ty()]),
          c_variadic: false,
          unsafety: Unsafety::Normal,
          abi: Abi::Rust,
        })
      }
      (FnResolution::Final(sub), FunctionKind::Closure) => sub.args.as_closure().sig(),
      (FnResolution::Final(sub), FunctionKind::Plain) => {
        sub.ty(tcx, ty::ParamEnv::reveal_all()).fn_sig(tcx)
      }
      (FnResolution::Partial(_), FunctionKind::Closure) => {
        if let Some(local) = def_id.as_local() {
          sess.span_warn(
            def_span,
            "Precise variable instantiation for \
                            closure not known, using user type annotation.",
          );
          let sig = tcx.closure_user_provided_sig(local);
          Ok(sig.value)
        } else {
          Err(sess.span_err(
            def_span,
            format!("Could not determine type signature for external closure {def_id:?}"),
          ))
        }?
      }
      (FnResolution::Partial(_), FunctionKind::Generator) => Err(sess.span_err(
        def_span,
        format!(
          "Cannot determine signature of generator {def_id:?} without monomorphization"
        ),
      ))?,
      (FnResolution::Partial(_), FunctionKind::Plain) => {
        let sig = tcx.fn_sig(def_id);
        sig.no_bound_vars().unwrap_or_else(|| {
                        sess.span_warn(def_span, format!("Cannot discharge bound variables for {sig:?}, they will not be considered by the analysis"));
                        sig.skip_binder()
                    })
      }
    };
    Ok(
      tcx
        .try_normalize_erasing_late_bound_regions(
          ty::ParamEnv::reveal_all(),
          late_bound_sig,
        )
        .unwrap_or_else(|e| {
          sess.span_warn(
            def_span,
            format!("Could not erase regions in {late_bound_sig:?}: {e:?}"),
          );
          late_bound_sig.skip_binder()
        }),
    )
  }
}

/// Try and normalize the provided generics.
///
/// The purpose of this function is to test whether resolving these generics
/// will return an error. We need this because [`ty::Instance::resolve`] fails
/// with a hard error when this normalization fails (even though it returns
/// [`Result`]). However legitimate situations can arise in the code where this
/// normalization fails for which we want to report warnings but carry on with
/// the analysis which a hard error doesn't allow us to do.
fn test_generics_normalization<'tcx>(
  tcx: TyCtxt<'tcx>,
  param_env: ParamEnv<'tcx>,
  args: &'tcx ty::List<ty::GenericArg<'tcx>>,
) -> Result<(), ty::normalize_erasing_regions::NormalizationError<'tcx>> {
  tcx
    .try_normalize_erasing_regions(param_env, args)
    .map(|_| ())
}

pub fn try_monomorphize<'tcx>(
  tcx: TyCtxt<'tcx>,
  def_id: DefId,
  param_env: ParamEnv<'tcx>,
  args: GenericArgsRef<'tcx>,
) -> FnResolution<'tcx> {
  let param_env = param_env.with_reveal_all_normalized(tcx);
  let make_opt = || {
    if let Err(e) = test_generics_normalization(tcx, param_env, args) {
      debug!("Normalization failed: {e:?}");
      return None;
    }
    Some(Instance::resolve(tcx, param_env, def_id, args).unwrap()?)
  };

  match make_opt() {
    Some(inst) => FnResolution::Final(inst),
    None => FnResolution::Partial(def_id),
  }
}
