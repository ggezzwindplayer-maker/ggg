# Rust `#![no_std]` en Xbox 360 (Xenon / PowerPC 32-bit big-endian) con libXenon

Proyecto mínimo para compilar Rust a la consola usando el toolchain open source
**libXenon** (`xenon-gcc`), con `core` y `compiler_builtins` reconstruidos vía
`-Z build-std`.

```
.
├── powerpc-unknown-xenon.json   # target spec de LLVM/rustc
├── .cargo/config.toml           # linker xenon-gcc + flags de enlace
├── rust-toolchain.toml          # nightly + rust-src
├── Cargo.toml
└── src/main.rs                  # #![no_std] #![no_main] + panic_handler
```

---

## 0. Aviso importante antes de empezar

El toolchain de libXenon **sólo se construye en Linux** (`build-xenon-toolchain`
es un script bash que compila binutils, GCC y newlib). En Windows hay dos rutas
reales:

* **WSL2** (recomendado): Ubuntu + el SDK dentro del WSL, y el proyecto en el
  filesystem de Linux (`~/…`, no en `/mnt/c/...`).
* **Docker**: `docker run -it -v "$PWD":/app free60/libxenon:latest`.

Cross-compilar *desde* Windows nativo no está soportado por libXenon.

Y una corrección de premisa: **libXenon no incluye `xenon-elf2xex`**. Su salida
es un ELF de PowerPC (`*.elf32`) que carga **XeLL**, no un `.xex`. Ver
[§5](#5-de-elf32-a-xex-la-realidad) para el detalle y las alternativas.

---

## 1. Instalar el toolchain libXenon (Linux / WSL2)

Dependencias del host (Debian/Ubuntu):

```bash
sudo apt-get update
sudo apt-get install -y build-essential flex bison gcc-multilib libgmp3-dev \
    libmpfr-dev libmpc-dev texinfo git-core wget file
```

Toolchain + librería (tarda ~1–2 h; GCC 16.2 + binutils 2.32 + newlib 3.1):

```bash
git clone https://github.com/Free60Project/libxenon.git
cd libxenon/toolchain
export PREFIX=/usr/local/xenon
./build-xenon-toolchain toolchain    # binutils + gcc + newlib
./build-xenon-toolchain libxenon     # libxenon.a + headers + app.lds
```

Variables de entorno (añádelas a `~/.bashrc`):

```bash
export DEVKITXENON=/usr/local/xenon
export PATH="$DEVKITXENON/bin:$DEVKITXENON/usr/bin:$PATH"
```

Comprobación:

```bash
xenon-gcc --version
xenon-ld --version
ls "$DEVKITXENON/app.lds" "$DEVKITXENON/usr/lib/libxenon.a"
```

Atajo con Docker (evita compilar GCC):

```bash
docker run -it -v "$PWD":/app free60/libxenon:latest
# dentro: export DEVKITXENON=/usr/local/xenon
```

---

## 2. Instalar Rust nightly + `rust-src`

`build-std` es nightly-only y necesita el código fuente de `library/`:

```bash
rustup toolchain install nightly --profile minimal --component rust-src
rustup override set nightly        # o confía en rust-toolchain.toml
rustup component list --installed  # debe aparecer rust-src
```

No hace falta `rustup target add`: el target es el JSON del repo y `core` se
recompila desde fuente.

---

## 3. Compilar

Con el alias definido en `.cargo/config.toml`:

```bash
cargo xenon            # debug
cargo xenon-release    # release (lto, opt-level=2)
```

Equivalente explícito (lo que hace el alias):

```bash
cargo build --release \
  --target powerpc-unknown-xenon \
  -Z build-std=core,compiler_builtins \
  -Z build-std-features=compiler-builtins-mem
```

Salida:

```
target/powerpc-unknown-xenon/release/xenon-hello
```

Ese fichero ya es un **ELF32 PowerPC big-endian** enlazado con
`$DEVKITXENON/app.lds` (base `0x80000000`), `libxenon.a`, newlib y libgcc, y con
`main` como punto de entrada C (`crt1.c` de libXenon llama a
`main(argc, argv)` y luego a `exit`).

Comprobación rápida del binario:

```bash
file target/powerpc-unknown-xenon/release/xenon-hello
# ELF 32-bit MSB executable, PowerPC or cisco 4500, version 1 (SYSV), statically linked
xenon-readelf -h target/powerpc-unknown-xenon/release/xenon-hello
# Class: ELF32   Data: 2's complement, big endian   Machine: PowerPC
xenon-nm target/powerpc-unknown-xenon/release/xenon-hello | grep -E ' (main|_start)$'
```

---

## 4. De ELF a la consola (ruta soportada: XeLL)

libXenon produce ELF, y XeLL lo carga desde un USB FAT32. Conversión al formato
que usan los Makefiles oficiales (`*.elf32`):

```bash
cd target/powerpc-unknown-xenon/release
xenon-objcopy -O elf32-powerpc xenon-hello xenon-hello.elf32
xenon-strip xenon-hello.elf32
```

> El `rules` de libXenon añade `--adjust-vma 0x80000000` porque su flujo enlaza
> en 0. Nosotros ya enlazamos en `0x80000000` con `app.lds`, así que **no** hay
> que añadirlo (lo duplicaría).

Copiar a la consola:

```bash
cp xenon-hello.elf32 /media/USB/xenon.elf    # el nombre debe ser xenon.elf
sync
```

Arrancar **XeLL** con el USB puesto: carga `xenon.elf` automáticamente.
Requiere consola con RGH/JTAG (o XeLL arrancable); no funciona en una consola
retail sin modificaciones.

---

## 5. De ELF32 a XEX: la realidad

`xenon-elf2xex` **no existe en libXenon**. Verificado sobre el repo actual:

* `toolchain/build-xenon-toolchain` sólo construye binutils, GCC, newlib,
  libxenon y libs auxiliares; no compila ninguna herramienta `elf2xex`.
* `devkitxenon/rules` define `%.elf` y `%.elf32` — no hay regla `%.xex`.
* El README de libXenon y los proyectos basados en él distribuyen `*.elf32`.

Un `.xex` no es “un ELF empaquetado”: el loader retail espera una imagen PE con
imports por ordinal de `xboxkrnl.exe`/`xam.xex` y base de imagen propia
(`0x82000000` en el caso de xsdk). Un ELF de libXenon está enlazado a
`0x80000000`, no importa nada del kernel y trae su propio runtime bare-metal;
empaquetarlo tal cual produce un XEX que el kernel no puede arrancar.

Alternativas reales:

| Ruta | Herramienta | Comentario |
|---|---|---|
| Homebrew clásico | XeLL + `xenon.elf` | **La ruta de libXenon.** Es la que funciona hoy. |
| XEX moderno | [xsdk](https://github.com/xenosworks/xsdk) | SDK distinto (GCC `powerpc-xsdk-elf`, newlib, `src/xsdk.ld`, imports del kernel) con `tools/elf2xex.py`: `python3 tools/elf2xex.py titulo.elf titulo.xex --title-name "..."`. No acepta un ELF de libXenon. |
| XEX con XDK/SDK filtrado | `xextool`/`XexTool` | Fuera del ámbito open source; requiere el modelo de imports del SDK. |

Si el objetivo es un `.xex` que corra desde el dashboard de una RGH, el camino
es un runtime con imports del kernel (xsdk), no libXenon. Si el objetivo es
código propio en la consola, `xenon.elf` + XeLL es exactamente lo que hace
libXenon.

---

## 6. Cómo está montado el target

`powerpc-unknown-xenon.json`:

| Campo | Valor | Por qué |
|---|---|---|
| `llvm-target` | `powerpc-unknown-none-elf` | PPC32 ELF, ABI SysV (no EABI: `xenon-gcc` se configuró sin `-meabi`, y EABI rompería los varargs de `printf`). |
| `data-layout` | `E-m:e-p:32:32-Fn32-i64:64-n32` | Idéntico al de `powerpc-unknown-linux-gnu` en rustc: big-endian, punteros de 32 bits alineados a 32, `i64` a 8, enteros nativos 32. |
| `target-pointer-width` | `32` (entero) | Formato actual de rustc (≥1.85). |
| `cpu` / `features` | `750` / `+altivec` | Xenon es un PPE in-order con VMX; `750` es el subconjunto seguro y `+altivec` habilita VMX128-compatible. LLVM no conoce `cell`. |
| `max-atomic-width` | `32` | El modo 32-bit no tiene `ld`/`std` atómicos; `AtomicU64` no debe existir. |
| `panic-strategy` | `abort` | Sin unwinder en bare metal. |
| `linker` / `linker-flavor` | `xenon-gcc` / `gcc` | El driver de GCC aporta `crt0.o`, `crti/crtbegin/crtend` de newlib. |
| `no-default-libraries` | `true` | Evita que rustc añada libs por defecto; las ponemos explícitas. |
| `crt-static-*`, `position-independent-executables` | `true` / `false` | Fuerza `-static` y evita `-pie`. |

Enlace (`.cargo/config.toml`) — replica `devkitxenon/rules`:

```
xenon-gcc -m32 -mcpu=cell -mtune=cell -maltivec -mpowerpc64 -fno-pic \
          -Wl,-n -Wl,--gc-sections \
          -Wl,-T,$DEVKITXENON/app.lds \
          -Wl,-u,_start -Wl,-u,read -Wl,-u,exc_base \
          -Wl,--start-group -L$DEVKITXENON/usr/lib -L$DEVKITXENON/xenon/lib/32 \
          -lxenon -lm -lc -lgcc -Wl,--end-group
```

`-u _start` / `-u exc_base` / `-u read` son los `-u` que usan los Makefiles de
libXenon para forzar la inclusión de `crt0`, la tabla de excepciones y el
soporte de lectura.

---

## 7. Troubleshooting

| Síntoma | Causa / solución |
|---|---|
| `custom targets are unstable and require -Zunstable-options` | Falta `-Z unstable-options` (ya está en `rustflags`) o se invocó `rustc` a mano sin él. |
| `can't find crate for 'core'` | Se compiló sin `-Z build-std=core,compiler_builtins`. Usa `cargo xenon`. |
| `error: linking with 'xenon-gcc' failed` + `cannot find -lxenon` | `DEVKITXENON` distinto de `/usr/local/xenon`: edita las rutas en `.cargo/config.toml`. |
| `undefined reference to '_start'` | `app.lds` no encontrado o `-nostartfiles` colado en `RUSTFLAGS`. Revisa que no haya `RUSTFLAGS` global. |
| `undefined reference to 'main'` | `#[unsafe(no_mangle)] pub extern "C" fn main` — sin `no_mangle` el símbolo se mangla y `crt1` no lo encuentra. |
| `undefined reference to 'memcpy'` | Debe resolverlo newlib/libXenon; si usas `-Z build-std-features=compiler-builtins-mem` y aun así falla, añade `-lc` (ya está). |
| El ELF arranca y cuelga en negro | `xenos_init()` no se llamó antes de `console_init()`, o el modo de vídeo no es válido para tu salida (prueba `VIDEO_MODE_AUTO`, `VIDEO_MODE_HDMI_720P`, etc.). |
| `error: environment variable CARGO_PKG_VERSION not defined` | Estás invocando `rustc` a mano; usa Cargo. |

---

## 8. Qué se verificó de esta plantilla

Con `rustc 1.102.0-nightly (9c5648833 2026-11-10)`:

* El JSON carga con `-Z unstable-options` y `--print cfg` reporta
  `target_arch="powerpc"`, `target_endian="big"`, `target_pointer_width="32"`,
  `target_os="none"`, `target_feature="altivec"`, `panic="abort"`,
  `target_has_atomic` = 8/16/32/ptr.
* `library/core` completa compila para este target (codegen PPC32 BE correcto).
* `src/main.rs` compila sin errores ni warnings y emite un objeto
  `ELF 32-bit MSB, machine 0x14 (PowerPC)`.
* La línea de enlace que genera rustc con `linker-flavor = "gcc"` para un bin
  `#![no_main]`/`no_std` **no** pasa `-nostartfiles` (sí `-nodefaultlibs`), por
  lo que el `crt0.o` de newlib aporta `_start` y `crt1.c` llama a `main`.

No verificable aquí: el enlace real contra `libxenon.a` (requiere el SDK
instalado en Linux) y la ejecución en hardware.
