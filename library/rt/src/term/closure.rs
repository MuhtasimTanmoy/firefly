use alloc::alloc::{Allocator, AllocError};
use core::any::TypeId;
use core::fmt;
use core::hash::{Hash, Hasher};

use seq_macro::seq;

use liblumen_alloc::gc::GcBox;

use crate::term::{Atom, OpaqueTerm};

extern {
    type ErlangException;
}

/// This type reflects the implicit return type expected by the Erlang calling convention
pub type ErlangResult = Result<OpaqueTerm, NonNull<ErlangException>>;

/// This struct unifies function captures and closures under a single type.
///
/// Closure contains all the metadata about the callee required to answer questions like
/// what is the arity, what module was it defined in, etc.
///
/// Closures (as opposed to function captures) have an implicit extra argument that comes first
/// in the argument list of the callee, which is a fat pointer to the Closure struct. This enables
/// the callee to access the closed-over values from its environment.
///
/// Function captures do not have the extra self argument, and always have an implicitly empty environment.
#[repr(C)]
pub struct Closure {
    pub module: Atom,
    pub name: Atom,
    pub arity: u8,
    fun: *const (),
    env: [OpaqueTerm],
}
impl fmt::Debug for Closure {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "#Fun<{}:{}/{}>", self.module, self.name, self.arity)
    }
}
impl Closure {
    pub const TYPE_ID: TypeId = TypeId::of::<Closure>();

    /// Allocates a new GcBox'd closure with the given name, callee, and environment, using the provided allocator
    ///
    /// # Safety
    ///
    /// This is a risky low-level operation, and is only safe if the following guarantees are upheld by the caller:
    ///
    /// * The callee pointer must point to an actual function
    /// * The callee must be guaranteed to outlive the closure itself
    /// * The callee must expect to receive `arity` arguments in addition to the closure self argument
    pub unsafe fn new<A: Allocator>(module: Atom, name: Atom, arity: u8, fun: *const (), env: &[OpaqueTerm], alloc: A) -> Result<GcBox<Self>, AllocError> {
        let mut this = GcBox::<Self>::with_capacity_in(env.len(), alloc)?;
        this.module = module;
        this.name = name;
        this.arity = arity;
        this.fun = fun;
        this.env.copy_from_slice(env);
        Ok(this)
    }

    /// Returns true if this closure is a function capture, i.e. it has no free variables.
    #[inline]
    pub fn is_thin(&self) -> bool {
        self.env.len() == 0
    }

    /// Applies the given slice of arguments to this closure.
    ///
    /// This function will panic if the number of arguments given does not match
    /// the arity of the closure.
    ///
    /// NOTE: Currently, a max arity of 10 is supported for dynamic apply via this function.
    /// If the number of arguments exceeds this number, this function will panic.
    #[inline]
    pub fn apply(&self, args: &[OpaqueTerm]) -> ErlangResult {
        seq!(N in 0..=10 {
            match args.len() {
                #(
                    N => apply~N(self, args),
                )*
                n => panic!("apply failed: too many arguments, got {}, expected no more than 10", n),
            }
        })
    }
}

seq!(A in 0..=10 {
    #(
        /// This type represents a function which implements a closure of arity A
        ///
        /// See the `Closure` docs for more information on how closures are implemented.
        pub type Closure~A = extern "C" fn (&Closure, seq!(N in 0..A { #(OpaqueTerm,)* })) -> ErlangResult;

        /// This type represents a function capture of arity A
        ///
        /// This differs from `ClosureA` in that a function capture has no implicit self argument.
        pub type Fun~A = extern "C" fn (seq!(N in 0..A { #(OpaqueTerm,)* })) -> ErlangResult;

        /// This type represents a tuple of A arguments
        pub type Args~A = seq!(N in 0..A { (#(OpaqueTerm,)*) });

        impl FnOnce<Args~A> for &Closure {
            type Output = ErlangResult;

            #[inline]
            extern "rust-call" fn call_once(self, args: Args~A) -> Self::Output {
                assert_eq!(self.arity, A, "mismatched arity");
                if self.is_thin() {
                    let fun = unsafe { core::mem::transmute::<_, Fun~A>(self.fun) };
                    fun(seq!(N in 0..A { #(args.N,)* }))
                } else {
                    let fun = unsafe { core::mem::transmute::<_, Closure~A>(self.fun) };
                    fun(self, seq!(N in 0..A { #(args.N,)* }))
                }
            }
        }
        impl FnMut<Args~A> for &Closure {
            #[inline]
            extern "rust-call" fn call_mut(&mut self, args: Args~A) -> Self::Output {
                assert_eq!(self.arity, A, "mismatched arity");
                if self.is_thin() {
                    let fun = unsafe { core::mem::transmute::<_, Fun~A>(self.fun) };
                    fun(seq!(N in 0..A { #(args.N,)* }))
                } else {
                    let fun = unsafe { core::mem::transmute::<_, Closure~A>(self.fun) };
                    fun(self, seq!(N in 0..A { #(args.N,)* }))
                }
            }
        }
        impl Fn<Args~A> for &Closure {
            #[inline]
            extern "rust-call" fn call(&self, args: Args~A) -> Self::Output {
                assert_eq!(self.arity, A, "mismatched arity");
                if self.is_thin() {
                    let fun = unsafe { core::mem::transmute::<_, Fun~A>(self.fun) };
                    fun(seq!(N in 0..A { #(args.N,)* }))
                } else {
                    let fun = unsafe { core::mem::transmute::<_, Closure~A>(self.fun) };
                    fun(self, seq!(N in 0..A { #(args.N,)* }))
                }
            }
        }

        /// Applies the given slice of arguments to a function of arity A
        ///
        /// NOTE: This function asserts that the length of `args` matches the arity of `fun`,
        /// if they do not match the function panics.
        #[inline]
        pub fn apply~A<F>(fun: F, args: &[OpaqueTerm]) -> ErlangResult
        where
            F: Fn(seq!(N in 0..A { #(OpaqueTerm,)* })) -> ErlangResult,
        {
            assert_eq!(args.len(), A, "mismatched arity");

            fun(seq!(N in 0..A { #(args[N],)* }))
        }
    )*
})

impl Eq for Closure {}
impl PartialEq for Closure {
    fn eq(&self, other: &Self) -> bool {
        self.module == other.module
            && self.name == other.name
            && self.arity == other.arity
            && std::ptr::eq(self.fun, other.fun)
    }
}
impl PartialOrd for Closure {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Closure {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        match self.module.cmp(&other.module) {
            Ordering::Equal => match self.name.cmp(&other.name) {
                Ordering::Equal => self.arity.cmp(&other.arity),
                other => other,
            },
            other => other,
        }
    }
}
impl Hash for Closure {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.module.hash(state);
        self.name.hash(state);
        self.arity.hash(state);
        ptr::hash(self.fun, state);
    }
}