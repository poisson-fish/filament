Continue the refactor plan in /home/filament/filament/plans/EVENT_REFACTOR.md.

Pick the next available unchecked task from the earliest phase that is not DONE.
If the task is too large, split it into micro-slices and execute only Slice 1 now.

Slice constraints:
- Keep it manageable: max 3-5 files
- Preserve security posture and existing limits/timeouts/rate caps.
- No unsafe Rust, no protocol-breaking changes unless explicitly allowed by the plan policy.
- Backward compatibility is optional for this refactor because the app is pre-deploy; if you simplify by breaking protocol compatibility, update server/client/docs/tests together in the same slice.

Execution rules:
1. State which plan item you selected.
2. Implement only one manageable slice.
3. Add/update focused tests for the slice.
4. Run relevant tests/lint for touched areas.
5. Update /home/filament/filament/plans/EVENT_REFACTOR.md with progress notes (and checkboxes if complete).

Return:
- What was implemented in this slice.
- Files changed.
- Tests run and results.
- Exact next slice to run after this.

Commit changes to the current branch when done.
