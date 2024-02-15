# freecut

freecut is a free and open source cut optimizer software for optimizing rectangular cut pieces from panels.
It is easy to use and after you have made the entries in the gui, a pdf file is created with the result.

This software is written in Rust using the Rust bindings for the FLTK Graphical User Interface library [fltk-rs](https://crates.io/crates/fltk), 
[comfy-table](https://crates.io/crates/comfy-table), [pdf-canvas](https://crates.io/crates/pdf-canvas) and the genetic algorithms and heuristics from the
[cut-optimizer-2d](https://crates.io/crates/cut-optimizer-2d) crate.

![Screenshot gui0](https://raw.githubusercontent.com/geri1701/freecut/main/screenshots/freecut_screenshot0.png)
![Screenshot gui1](https://raw.githubusercontent.com/geri1701/freecut/main/screenshots/freecut_screenshot1.png)
![Screenshot pdf](https://raw.githubusercontent.com/geri1701/freecut/main/screenshots/freecut_screenshot2.png)

## Installation

### Linux and other

First install `cargo` and `cmake`, which is a dependency of fltk-sys.

Now, compile the freecut-crate:

```
cargo install freecut
```
## Usage

This software helps you to optimize panel cuts.

### Add a stockpiece

To add a stockpiece, fill all fields and press the "add" Button, the stockpiece will apear in the table 
in the output fields.

### Add a cutpiece

To add a cutpiece, fill all fields and press the "add" Button, the cutpiece will apear in the table in the 
output.

### Pattern

If a pattern on the workpiece is to be taken into account, then select the respective direction.
In this case, however, a pattern must also be selected on each cutpiece. 

### Optimize

Choose a cutwidth between 1 and 15mm and a prefered Layout.
Guillotine-Layout is better for panel-saws.
Now press the [optimize]-Button and a pdf-File with a solution will be generated.

## Contributions

Contributions are welcome, please create an issue or pull request.
