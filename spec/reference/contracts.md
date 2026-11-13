# Contracts

Contracts state what a function promises; tests and examples check it.

r[contract.runtime]
Contracts are checked at runtime in debug and test builds, and in release builds unless a function opts out with `@unchecked` after profiling.

r[contract.cost-warn]
Because contracts usually run in production, quince check warns about any contract that calls a standard-library function documented as slower than constant time, such as xs.sorted(), or a function of your own that contains a loop or recursion.

## `requires` and `ensures`

```ebnf
contract     = "requires" expr | "ensures" [ "(" name ")" ] expr
```

```
pub fn withdraw(account: Account, amount: Money) -> Result<Account, WithdrawError>
  requires amount > Money.ZERO
  ensures(r) match r {
    Ok(a) => a.balance == account.balance - amount,
    Err(_) => true,
  }
{ ... }
```

r[contract.requires]
- `requires` constrains the caller; a violation panics and blames the call site.

r[contract.ensures]
- `ensures` constrains the function; a violation panics and blames the function.

r[contract.ensures-name]
- `ensures(r)` names the return value `r` (any name works) for use in the condition.

r[contract.ensures-binder]
- `ensures` followed by a name in parentheses always names the return value. A condition that starts with a parenthesized name is written without the parentheses.

r[contract.pure]
- Contract expressions must be pure: no effects and no mutation. Reading a reference type needs `uses State`, so a contract can't read shared state such as `stats.get()`; it constrains only arguments and the return value.

r[contract.pub-visibility]
- The contracts of a `pub` function may name only what its callers can see: its parameters, `pub` fields and `pub` functions. Otherwise it is a compile error, and the suggested fix adds a `pub` pure accessor. A `requires` blames the caller, so the caller must be able to read it through `quince` `query signature`. Invariants on private fields belong in the struct's own `requires`.

r[contract.var-self]
- In a method that takes `var self`, `ensures` sees `self` as it is after the call.

r[contract.no-old]
- There is no way to name the value from before the call, since there is no `old()`.

r[contract.invariant]
- Struct invariants use `requires` on the struct and are checked after every construction and update: `pub struct Interval { pub start: Int, pub end: Int } requires start <= end`.

r[contract.invariant-update]
- Each assignment to a field of a var binding is a separate update, so a change that moves two fields together is written as one: `r = Interval(..r, start: 10, end: 20)`.

r[contract.iface]
- Interface methods may declare `requires` and `ensures`.

r[contract.iface-inherit]
- Every `impl` inherits them unchanged and can't add or remove any, so a method's contract is always read in one place: the interface.

## Static verification (optional)

r[contract.verify.opt-in]
`quince` `verify` attempts to prove contracts with an SMT solver. It is opt-in per function with `@verify`, reports which contracts it proved, and never blocks a build unless the project config makes it required.
