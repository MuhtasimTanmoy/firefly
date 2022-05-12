mod dynamic;

use std::ffi::c_void;
use std::lazy::SyncOnceCell;
use std::mem;

use hashbrown::{HashMap, HashSet};

use liblumen_arena::DroplessArena;
use liblumen_core::symbols::FunctionSymbol;

use crate::erts::process::ffi::ErlangResult;
use crate::erts::term::prelude::Atom;
use crate::erts::term::prelude::Term;
use crate::erts::ModuleFunctionArity;
use liblumen_core::alloc::Layout;

pub use self::dynamic::DynamicCallee;

/// Dynamically invokes the function mapped to the given symbol.
///
/// - The caller is responsible for making sure that the given symbol
/// belongs to a function compiled into the executable.
/// - The caller must ensure that the target function adheres to the ABI
/// requirements of the destination function:
///   - C calling convention
///   - Accepts only immediate-sized terms as arguments
///   - Returns an immediate-sized term as a result
///
/// This function returns `Err` if the called function returns the NONE value,
/// or if the given symbol doesn't exist.
///
/// This function will panic if the symbol table has not been initialized.
pub unsafe fn apply(symbol: &ModuleFunctionArity, args: &[Term]) -> Result<ErlangResult, ()> {
    if let Some(f) = find_symbol(symbol) {
        Ok(dynamic::apply(f, args.as_ptr(), args.len()))
    } else {
        Err(())
    }
}

pub unsafe fn apply_callee(callee: DynamicCallee, args: &[Term]) -> ErlangResult {
    dynamic::apply(callee, args.as_ptr(), args.len())
}

pub fn find_symbol(mfa: &ModuleFunctionArity) -> Option<DynamicCallee> {
    let symbols = SYMBOLS.get().unwrap_or_else(|| {
        panic!(
            "InitializeLumenDispatchTable not called before trying to get {:?}",
            mfa
        )
    });
    if let Some(f) = symbols.get_function(mfa) {
        Some(unsafe { mem::transmute::<*const c_void, DynamicCallee>(f) })
    } else {
        None
    }
}

pub fn dump_symbols() {
    SYMBOLS.get().map(|symbols| symbols.dump());
}

pub fn module_loaded(module: Atom) -> bool {
    let symbols = SYMBOLS.get().unwrap_or_else(|| {
        panic!(
            "InitializeLumenDispatchTable not called before trying to check module ({}) loaded",
            module
        )
    });

    symbols.contains_module(module)
}

/// The symbol table used by the runtime system
static SYMBOLS: SyncOnceCell<SymbolTable> = SyncOnceCell::new();

/// Performs one-time initialization of the atom table at program start, using the
/// array of constant atom values present in the compiled program.
///
/// It is expected that this will be called by code generated by the compiler, during the
/// earliest phase of startup, to ensure that nothing has tried to use the atom table yet.
#[no_mangle]
pub unsafe extern "C" fn InitializeLumenDispatchTable(
    start: *const FunctionSymbol,
    end: *const FunctionSymbol,
) -> bool {
    if start.is_null() || end.is_null() {
        return false;
    }

    let len = end.offset_from(start);
    let mut table = SymbolTable::new(len.try_into().unwrap());
    let mut next = start;
    loop {
        if next >= end {
            break;
        }
        let symbol = &*next;
        let module = Atom::from_raw_cstr(symbol.module as *const std::os::raw::c_char);
        let function = Atom::from_raw_cstr(symbol.function as *const std::os::raw::c_char);
        let arity = symbol.arity;
        let callee = symbol.ptr;

        let size = mem::size_of::<ModuleFunctionArity>();
        let align = mem::align_of::<ModuleFunctionArity>();
        let layout = Layout::from_size_align(size, align).unwrap();
        let ptr = table.arena.alloc_raw(layout) as *mut ModuleFunctionArity;
        ptr.write(ModuleFunctionArity {
            module,
            function,
            arity,
        });
        let sym = mem::transmute::<&ModuleFunctionArity, &'static ModuleFunctionArity>(&*ptr);
        assert_eq!(None, table.idents.insert(callee, sym));
        assert_eq!(None, table.functions.insert(sym, callee));
        table.modules.insert(sym.module);

        next = next.add(1);
    }

    if let Err(_) = SYMBOLS.set(table) {
        eprintln!("tried to initialize symbol table more than once!");
        false
    } else {
        true
    }
}

struct SymbolTable {
    functions: HashMap<&'static ModuleFunctionArity, *const c_void>,
    idents: HashMap<*const c_void, &'static ModuleFunctionArity>,
    modules: HashSet<Atom>,
    arena: DroplessArena,
}
impl SymbolTable {
    fn new(size: usize) -> Self {
        Self {
            functions: HashMap::with_capacity(size),
            idents: HashMap::with_capacity(size),
            modules: HashSet::new(),
            arena: DroplessArena::default(),
        }
    }

    fn dump(&self) {
        eprintln!("START SymbolTable at {:p}", self);
        for mfa in self.functions.keys() {
            eprintln!("{:?}", mfa);
        }
        eprintln!("END SymbolTable");
    }

    #[allow(unused)]
    fn get_ident(&self, function: *const c_void) -> Option<&'static ModuleFunctionArity> {
        self.idents.get(&function).copied()
    }

    fn get_function(&self, ident: &ModuleFunctionArity) -> Option<*const c_void> {
        self.functions.get(ident).copied()
    }

    fn contains_module(&self, module: Atom) -> bool {
        self.modules.contains(&module)
    }
}

// These are safe to implement because the items in the symbol table are static
unsafe impl Sync for SymbolTable {}
unsafe impl Send for SymbolTable {}