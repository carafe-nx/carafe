# 46. The system-call bridge leaves as a return

Date: 2026-10-29

Builds on [decision 29](0029-fex-built-with-carafe-patches.md); stage 4 of the orchestrator research, item b2 of
[a57-translation](../research/orchestrator/findings/a57-translation.md).

## Context

FEX predicts guest returns with a call-return stack: a call pushes the guest return address and the host code after
the call, and a return that matches the top goes straight back. Every system call and unix call of a 32-bit game enters
FEX's WoW64 bridge through a call (`ntdll.dll`'s stubs, `__wine_unix_call` from `winevulkan.dll` for each Vulkan
command) and goes on at the return address without a return, so the call's entry stays on the stack. From then on each
return of the outer frames pops the entry meant for the frame inside it and misses. In NFSU2's title menu in the lab,
25–50 % of the main thread's returns missed, 11 million in one run; a recorder showed the popped addresses were the
return addresses right after those calls.

## Decision

- **FEX patch 0017, `BridgeReturn`.** The bridge's exit looks at the top of the call-return stack: when it holds the
  return address the bridge goes on at, it pops it and returns to its host code; otherwise the stack is left alone
  and the exit takes the lookup as before.
- **Autorun patch 0073** adds the option, and FEX patch 0016's `IndirectCallCache`, to the list of FEX options the
  runtime passes on; options outside that list never reach FEX. `FEX_BRIDGERETURN` is on unless a program turns it
  off.
- **Built in, without a row.** In the lab, NFSU2's main thread missed 89 thousand returns with it against 11 million
  without it, the other threads went from up to 0.9 million to under a thousand, and the games ran the same steps with
  the same pictures. On the console, Carbon's main thread went from about 24 million misses per report to 20
  thousand, and its driving from 46–56 frames a second to 53–59 at the same 82 % of GPU load (different routes).
  The change only acts when the top of the stack holds the bridge's return address, so there is nothing to turn off.
