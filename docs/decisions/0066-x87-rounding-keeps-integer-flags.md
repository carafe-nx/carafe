# x87 rounding keeps integer flags

## Context

FEX patch `0006` lowers reduced-precision `FRNDINT` to four vector rounding
operations and selects the result using the x87 control word. Its integer
comparisons overwrite live guest arithmetic flags. `FRNDINT` must preserve those
flags. In Double Agent, a following `JL` then continues a loop past its stack
buffer and corrupts the exception-handler chain.

The nearest candidate also used the host rounding mode, which follows SSE state.
The x87 and SSE rounding modes are independent.

The affected source is
`FEXCore/Source/Interface/IR/Passes/x87StackOptimizationPass.cpp`, the
`OP_F80ROUNDSTACK` lowering around lines 1365–1384 after Carafe patches, based on
FEX revision `395b132f346b1a45def246d10c52245edba1ef02`.

## Decision

Patch `0025` saves and restores NZCV around the selection comparisons and uses
explicit nearest rounding for x87 mode zero. Keep the existing optimization and
its configuration.

The Windows ARM64 regression executable runs actual 32-bit guest instructions
through FEX. It checks rounding, all six arithmetic flags and a conditional jump
for 672 combinations of x87 mode, SSE mode, input and preceding comparison.
Run it with arguments `0 1000`, `1 1000` and `0 1` to exercise precision settings
and block boundaries. Each configuration fails 520 cases before the fix and
passes all cases after it.

## Rejected options

- Disabling conversion rounding globally removes the optimization without fixing
  its instruction semantics.
- Changing the game's loop or exception handling hides translated-code
  corruption.
- Changing SaveData paths does not address the reproduced loop and stack damage.

## Validation limits

The regression establishes FEX instruction behavior on Windows ARM64. With the
fixed FEX and the ordinary native runtime, Double Agent reaches Training Test
Course A in Lab, responds to movement and writes a save that survives NAND
export. The previous `D3DDrv+0x87f0c` exception does not recur in this pass.

On the physical console, the user confirms that Training loads and saving works.
New Game still fails: DXVK cannot create an anonymous memory section, followed
by a null-pointer read in `D3DDrv+0xb8e2d`. One console session also records two
failed Vulkan allocations of 128 MB each.

In Lab, New Game stops with `STATUS_NO_MEMORY` on an 8196 KB allocation, and the
image in Training is overexposed. The precise reasons for these allocation
failures and the overexposure are not established by this change.
