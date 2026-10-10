# 1. Build on Autorun

Date: 2026-10-04

## Context

A Windows game needs Wine, an x86 emulator and a graphics stack to run on the Switch. Autorun is the only working Wine
for Horizon OS: it brings Wine, Box64 and FEX for x86 code, DXVK for Direct3D and Mesa for the GPU. Proton and FEX on
their own are Linux programs; moving them to Horizon would repeat Autorun's work.

## Decision

Every NSP carries Autorun's runtime, pinned to one commit (`external/autorun`). Carafe adds its own loader, an overlay
compiled into the runtime and a set of patches (see [6](0006-patches-not-forks.md)). FEX is already part of Autorun,
so "FEX is better than Box64" is not a reason to leave it.

## Rejected

- L4T Linux: Proton and FEX run there, but the result is not a standalone NSP.
- Porting Proton to Horizon now: a separate research project, listed in the roadmap.
