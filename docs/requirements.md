# Freecut requirements

## Context

The original Freecut proved that a Rust desktop GUI can collect rectangular cut lists, optimize them, and produce a PDF result. The new project should start from what users actually asked for instead of only reproducing the original implementation.

## Initial user-facing goals

- Maintain editable stock-piece and cut-piece lists.
- Import cut lists from CSV.
- Modify cut-piece dimensions and quantities after entry.
- Optimize rectangular cuts with kerf width and layout constraints.
- Show a graphical solution inside the application.
- Export the same solution to PDF and possibly later SVG/PNG.

## Explicit goals carried over from old Freecut issue feedback

Freecut must satisfy these three user-facing requests from the old Freecut project:

- Import a CSV file.
- Modify cut-piece dimensions on the go.
- Show a graphical view of the solution inside the software.

## Important product constraints

- The graphical solution must be based on the same solution model as export.
- The optimizer output must remain connected to the user's original pieces.
- The application should be able to explain or at least inspect how a result maps back to input rows.
- The internal model should not depend on a third-party optimizer's public result structs.

## Decisions

- GUI toolkit: egui/eframe.
- First optimizer path: Freecut-owned deterministic optimizer with `Fast`, `Balanced`, and `Thorough` effort levels.

## Current persistence and import scope

- Native project files are explicit JSON documents using the `.freecut.json` suffix.
- CSV import is data-only: it imports stock and cut rows, but not project settings such as unit, kerf width, layout, or optimizer effort.
