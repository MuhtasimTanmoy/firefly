#ifndef LUMEN_MODULEBUILDER_SUPPORT_H
#define LUMEN_MODULEBUILDER_SUPPORT_H

#include <stdint.h>

#include "llvm/Support/Casting.h"
#include "lumen/mlir/IR.h"

// Recursively searches for the Operation which defines the given value
Operation *getDefinition(Value val) {
  if (auto arg = val.dyn_cast_or_null<BlockArgument>()) {
    Block *block = arg.getOwner();
    // If this block is the entry block, then we can't get the definition
    if (block->isEntryBlock())
      return nullptr;
    // If this block has no predecessors, then we can't get the definition
    if (block->hasNoPredecessors())
      return nullptr;
    // If there is a single predecessor, check the value passed as argument
    // to this block.
    //
    // If this block has multiple predecessors, we need to check if the
    // argument traces back to a single value; otherwise there are different
    // values in different branches, and we can't get a single definition
    Operation *result = nullptr;
    for (Block *pred : block->getPredecessors()) {
      auto index = arg.getArgNumber();
      Operation *found = nullptr;
      pred->walk([&](BranchOpInterface branchInterface) {
        auto op = branchInterface.getOperation();
        for (auto it = pred->succ_begin(), e = pred->succ_end(); it != e; ++it) {
          // If the successor isn't our block, we don't care
          Block *succ = *it;
          if (block != succ)
            continue;
          // No operands, nothing to do
          auto maybeSuccessorOperands =
            branchInterface.getSuccessorOperands(it.getIndex());
          if (!maybeSuccessorOperands.hasValue())
            continue;
          // Otherwise take a look at the value passed as the successor block argument
          auto successorOperands = maybeSuccessorOperands.getValue();
          Value candidate = successorOperands[index];
          Operation *def = getDefinition(candidate);
          if (found && def != found)
            return WalkResult::interrupt();
          found = def;
        }
      });
      // If this result doesn't match the last, we've found a conflict
      if (result && found != result)
        return nullptr;
      result = found;
    }

    return nullptr;
  }
  return val.getDefiningOp();
}

template <typename OpTy>
OpTy getDefinition(Value val) {
  return llvm::dyn_cast_or_null<OpTy>(getDefinition(val));
}

namespace lumen {
namespace eir {

//===----------------------------------------------------------------------===//
// Location Metadata
//===----------------------------------------------------------------------===//

/// A source span
struct Span {
  // The starting byte index of a span
  uint32_t start;
  // The end byte index of a span
  uint32_t end;
};

/// A source location
struct SourceLocation {
  const char *filename;
  uint32_t line;
  uint32_t column;
};

//===----------------------------------------------------------------------===//
// Types
//===----------------------------------------------------------------------===//

/// An enum which maps term kinds used in encoding to values
/// passed to the builder from Rust.
///
/// This is NOT the same thing as the type kind in the EIR dialect,
/// but can be mapped to a type kind
namespace EirTypeTag {
enum TypeTag {
#define EIR_TERM_KIND(Name, Val) Name = Val,
#define FIRST_EIR_TERM_KIND(Name, Val) EIR_TERM_KIND(Name, Val)
#include "lumen/EIR/IR/EIREncoding.h.inc"
};
}  // namespace EirTypeTag

// Representation of the Type enum defined in Rust

struct EirTypeAny {
  EirTypeTag::TypeTag tag;
};
struct EirTypeTuple {
  EirTypeTag::TypeTag tag;
  unsigned arity;
};

union EirType {
  EirTypeAny any;
  EirTypeTuple tuple;
};

//===----------------------------------------------------------------------===//
// Functions/Blocks
//===----------------------------------------------------------------------===//

/// The result of declaring a new function
///
/// Contains the function value, as well as the entry block
struct FunctionDeclResult {
  MLIRFunctionOpRef function;
  MLIRBlockRef entryBlock;
};

// Used to represent function/block arguments
struct Arg {
  EirType ty;
  Span span;
  bool isImplicit;
};

//===----------------------------------------------------------------------===//
// Maps and MapUpdate/MapAction
//===----------------------------------------------------------------------===//

enum class MapActionType : uint32_t { Unknown = 0, Insert, Update };

struct MapAction {
  MapActionType action;
  MLIRValueRef key;
  MLIRValueRef value;
};

struct MapUpdate {
  MLIRLocationRef loc;
  MLIRValueRef map;
  MLIRBlockRef ok;
  MLIRBlockRef err;
  MapAction *actionsv;
  size_t actionsc;
};

// Used to represent map key/value pairs used in map construction
struct MapEntry {
  MLIRValueRef key;
  MLIRValueRef value;
};

// Used to represent map key/value pairs used in constant maps
struct KeyValuePair {
  MLIRAttributeRef key;
  MLIRAttributeRef value;
};

//===----------------------------------------------------------------------===//
// Closures
//===----------------------------------------------------------------------===//

// Represents a captured closure, possibly with no environment
struct Closure {
  MLIRLocationRef loc;
  MLIRAttributeRef module;
  char *name;
  uint8_t arity;
  uint32_t index;
  uint32_t oldUnique;
  char unique[16];
  MLIRValueRef *env;
  unsigned envLen;
};

//===----------------------------------------------------------------------===//
// Binary Support Types
//===----------------------------------------------------------------------===//

namespace Endianness {
enum Type : uint32_t {
  Big,
  Little,
  Native,
};
}

namespace BinarySpecifierType {
enum Type : uint32_t {
  Integer,
  Float,
  Bytes,
  Bits,
  Utf8,
  Utf16,
  Utf32,
};
}

struct IntegerSpecifier {
  bool isSigned;
  Endianness::Type endianness;
  int64_t unit;
};

struct FloatSpecifier {
  Endianness::Type endianness;
  int64_t unit;
};

struct UnitSpecifier {
  int64_t unit;
};

struct EndiannessSpecifier {
  Endianness::Type endianness;
};

union BinarySpecifierPayload {
  IntegerSpecifier i;
  FloatSpecifier f;
  UnitSpecifier us;
  EndiannessSpecifier es;
};

struct BinarySpecifier {
  BinarySpecifierType::Type tag;
  BinarySpecifierPayload payload;
};

//===----------------------------------------------------------------------===//
// MatchOp Support Types
//===----------------------------------------------------------------------===//

enum class MatchPatternType : uint32_t {
  Any,
  Cons,
  Tuple,
  MapItem,
  IsType,
  Value,
  Binary,
};

class MatchPattern {
 public:
  MatchPattern(MatchPatternType tag) : tag(tag) {}

  MatchPatternType getKind() const { return tag; }

 private:
  MatchPatternType tag;
};

class AnyPattern : public MatchPattern {
 public:
  AnyPattern() : MatchPattern(MatchPatternType::Any) {}

  static bool classof(const MatchPattern *pattern) {
    return pattern->getKind() == MatchPatternType::Any;
  }
};

class ConsPattern : public MatchPattern {
 public:
  ConsPattern() : MatchPattern(MatchPatternType::Cons) {}

  static bool classof(const MatchPattern *pattern) {
    return pattern->getKind() == MatchPatternType::Cons;
  }
};

class TuplePattern : public MatchPattern {
 public:
  TuplePattern(unsigned arity)
      : MatchPattern(MatchPatternType::Tuple), arity(arity) {}

  unsigned getArity() const { return arity; }

  static bool classof(const MatchPattern *pattern) {
    return pattern->getKind() == MatchPatternType::Tuple;
  }

 private:
  unsigned arity;
};

class MapPattern : public MatchPattern {
 public:
  MapPattern(Value key) : MatchPattern(MatchPatternType::MapItem), key(key) {}

  Value getKey() { return key; }

  static bool classof(const MatchPattern *pattern) {
    return pattern->getKind() == MatchPatternType::MapItem;
  }

 private:
  Value key;
};

class IsTypePattern : public MatchPattern {
 public:
  IsTypePattern(Type ty)
      : MatchPattern(MatchPatternType::IsType), expectedType(ty) {}

  Type getExpectedType() { return expectedType; }

  static bool classof(const MatchPattern *pattern) {
    return pattern->getKind() == MatchPatternType::IsType;
  }

 private:
  Type expectedType;
};

class ValuePattern : public MatchPattern {
 public:
  ValuePattern(Value value)
      : MatchPattern(MatchPatternType::Value), value(value) {}

  Value getValue() { return value; }

  static bool classof(const MatchPattern *pattern) {
    return pattern->getKind() == MatchPatternType::Value;
  }

 private:
  Value value;
};

class BinaryPattern : public MatchPattern {
 public:
  BinaryPattern(BinarySpecifier spec, llvm::Optional<Value> size = llvm::None)
      : MatchPattern(MatchPatternType::Binary), spec(spec), size(size) {}

  llvm::Optional<Value> getSize() { return size; }
  BinarySpecifier &getSpec() { return spec; }

  static bool classof(const MatchPattern *pattern) {
    return pattern->getKind() == MatchPatternType::Binary;
  }

 private:
  llvm::Optional<Value> size;
  BinarySpecifier spec;
};

struct MLIRBinaryPayload {
  MLIRValueRef size;
  BinarySpecifier spec;
};

union MLIRMatchPatternPayload {
  unsigned i;
  MLIRValueRef v;
  EirType t;
  MLIRBinaryPayload b;
};

struct MLIRMatchPattern {
  MatchPatternType tag;
  MLIRMatchPatternPayload payload;
};

// Represents a single match arm
struct MLIRMatchBranch {
  MLIRLocationRef loc;
  MLIRBlockRef dest;
  MLIRValueRef *destArgv;
  unsigned destArgc;
  MLIRMatchPattern pattern;
};

// Represents a match operation
struct Match {
  MLIRLocationRef loc;
  MLIRValueRef selector;
  MLIRMatchBranch *branches;
  unsigned numBranches;
};

}  // namespace eir
}  // namespace lumen

#endif  // LUMEN_MODULEBUILDER_SUPPORT_H
