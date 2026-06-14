# vega68

a console emulator using a motorola 68k-like cpu. vega68 code is written
in freestanding zig.

## building

```sh
zig build          # build the emulator (vega68) + asset tool (vega68-pack)
zig build run      # run the emulator
zig build test     # m68k cpu tests
```

> [!NOTE]
> m68k cpu tests: [MAME](https://github.com/SingleStepTests/m68000) + a [musashi](https://github.com/kstenerud/Musashi) generated set

## devkit example rom

```sh
zig build rom      # build the hello example rom
```

> **Dependencies**
> - **GNU binutils** (`m68k-elf-as` + `m68k-elf-ld` + `m68k-elf-objcopy`)
> - **zig v0.16.0** (built with m68k llvm backend)

## [license](LICENSE)

```
MIT License

Copyright (c) 2026 asrael

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
