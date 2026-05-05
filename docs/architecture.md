# Freecut architecture

## Core decision

Freecut owns its domain and solution model. Optimization implementations are replaceable details behind an internal boundary.

The old design used an external optimizer as the center of gravity: input was adapted into the library and the returned `Solution` was rendered directly to PDF. That was enough for the prototype, but it makes editing, preview, import, and richer cut planning awkward.

## Layers

### Domain

Owns project input:

- units
- stock pieces
- cut pieces
- quantities
- pattern direction
- rotation rules
- kerf width
- layout preference

This layer must not know about GUI widgets, PDF primitives, or external optimizer result structs.

### Optimizer

Consumes `domain::Project` and returns `render::Solution` or a future richer internal solution type.

The implementation may use known bin-packing/cut-optimization heuristics, including ideas from permissively licensed sources, but Freecut should own the representation and integration points.

Current internal backends:

- `Guillotine`: deterministic guillotine packing for saw-friendly rectangular layouts.
- `Nested`: deterministic, rectangular, non-guillotine MaxRects-like packing. This is not polygon nesting and does not expose a cutting path.

Both backends stay behind the private optimizer boundary. UI, export, import, and project I/O see only `domain::Project`, `OptimizerConfig`, and `render::Solution`; egui types must not enter optimizer internals.

### Render/Solution

Owns placed geometry:

- sheets
- placed cut-piece instances
- waste rectangles
- source IDs back to input pieces
- optional fitness/quality metadata

GUI preview, PDF export, and later SVG/PNG export should all consume this model.

A correct guillotine cutting guide is a Slicing Tree of executable decomposition cuts:

- inner nodes are horizontal or vertical cuts on their current work rectangle,
- leaf nodes are final cut pieces or waste/offcuts,
- execution of an existing guide is a pre-order traversal,
- the GUI draws the kerf geometry of each cut; colors, labels, and numbering are presentation only.

Instructional guillotine cutlines must not be inferred from final piece edges or waste borders. If an executable guillotine cutting guide is not available, the UI should not invent one; Nested may still color actual uncovered kerf/gap geometry as a red seam without treating it as a guillotine guide.

### Import

CSV import should translate external rows into `domain::Project` changes. The CSV schema should be explicit and documented before implementation.

### UI

The UI is built with egui/eframe. It edits `domain::Project`, triggers optimization, and displays `render::Solution`. It should not store optimizer-private objects as application state, and egui types must not leak into `domain`, `optimizer`, `render`, or `import`.

## Public boundaries

The public application boundary is intentionally small: users edit projects, import CSV data, run optimization, inspect the graphical result, and export PDFs. Internal optimizer candidates, repair stages, and cutting-guide construction remain implementation details.
