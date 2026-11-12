//! Port-only "Opções" screen (rising_options.h, extension v1). Additive: no
//! existing function, structure or shell behaviour changes. An options handle
//! holds the host's values and the decoded menu background; every call that
//! draws or hit-tests reads a prepared shell session (fonts, MENU.CMP, mode)
//! without changing it, its surface or its generation.
use super::{shell_abi, status, with_registry, AbiError, Result};
use rl_core::options_screen::{self, Event, Input, Item, Language, Look, Options, Values};
use std::{
    collections::BTreeMap,
    ptr, slice,
    sync::{Mutex, OnceLock},
};

const VERSION: u32 = 1;
const PIXELS: usize = options_screen::PIXELS;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RlOpcoesValores {
    pub version: u32,
    pub atalhos: u32,
    pub caminhos: u32,
    pub dificuldade: u32,
    pub velocidade: u32,
    pub disponivel: u32,
    pub idioma: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RlOpcoesEntrada {
    pub version: u32,
    pub keys: u32,
    pub mouse_x: u32,
    pub mouse_y: u32,
    pub mouse_ae: u32,
    pub mouse_af: u32,
    pub ticks: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RlOpcoesEvento {
    pub kind: u32,
    pub consumed: u32,
    pub item: u32,
    pub open: u32,
    pub values: RlOpcoesValores,
}
const KEY_ENTER: u32 = 1;
const KEY_UP: u32 = 2;
const KEY_DOWN: u32 = 4;
const KEY_ESCAPE: u32 = 8;
const KEY_LEFT: u32 = 16;
const KEY_RIGHT: u32 = 32;
const NO_ITEM: u32 = u32::MAX;

fn values(v: &RlOpcoesValores) -> Result<(Values, Language)> {
    if v.version != VERSION
        || v.atalhos > 1
        || v.caminhos > 1
        || v.idioma > 1
        || v.disponivel > u32::from(options_screen::ALL)
        || v.reserved != 0
    {
        return Err(AbiError::InvalidArgument);
    }
    let out = Values {
        hotkeys: v.atalhos != 0,
        paths: v.caminhos != 0,
        difficulty: u8::try_from(v.dificuldade).map_err(|_| AbiError::InvalidArgument)?,
        speed: u8::try_from(v.velocidade).map_err(|_| AbiError::InvalidArgument)?,
        available: v.disponivel as u8,
    };
    out.validate().map_err(|_| AbiError::InvalidArgument)?;
    let language = if v.idioma == 0 {
        Language::PortugueseBr
    } else {
        Language::English
    };
    Ok((out, language))
}
fn c_values(o: &Options) -> RlOpcoesValores {
    RlOpcoesValores {
        version: VERSION,
        atalhos: u32::from(o.values.hotkeys),
        caminhos: u32::from(o.values.paths),
        dificuldade: u32::from(o.values.difficulty),
        velocidade: u32::from(o.values.speed),
        disponivel: u32::from(o.values.available),
        idioma: o.language as u32,
        reserved: 0,
    }
}

static OPTIONS: OnceLock<Mutex<BTreeMap<u64, Options>>> = OnceLock::new();
fn registry<T>(f: impl FnOnce(&mut BTreeMap<u64, Options>) -> Result<T>) -> Result<T> {
    let mut r = OPTIONS
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .map_err(|_| AbiError::Poisoned)?;
    f(&mut r)
}
/// Runs `f` on a COPY of the state and publishes it only on success, so every
/// error leaves the handle unchanged. Lock order: options, then shell sessions.
fn with_options<T>(handle: u64, f: impl FnOnce(&mut Options) -> Result<T>) -> Result<T> {
    registry(|r| {
        let current = r.get(&handle).ok_or(AbiError::InvalidHandle)?;
        let mut next = current.clone();
        let out = f(&mut next)?;
        r.insert(handle, next);
        Ok(out)
    })
}
fn with_look<T>(shell: u64, f: impl FnOnce(&Look<'_>, bool) -> Result<T>) -> Result<T> {
    shell_abi::session_view(shell, |memory, renderer, banks, pending| {
        let look = Look {
            memory,
            fonts: &renderer.assets,
            menu: banks.menu.assets(),
        };
        f(&look, pending)
    })
}
fn render(e: rl_core::render::Error) -> AbiError {
    shell_abi::render_error(e)
}
unsafe fn read<T: Copy>(p: *const T) -> Result<T> {
    if p.is_null() || !(p as usize).is_multiple_of(std::mem::align_of::<T>()) {
        return Err(AbiError::InvalidArgument);
    }
    Ok(unsafe { p.read() })
}
unsafe fn write<T>(p: *mut T, value: T) -> Result<()> {
    if p.is_null() || !(p as usize).is_multiple_of(std::mem::align_of::<T>()) {
        return Err(AbiError::InvalidArgument);
    }
    unsafe { p.write(value) };
    Ok(())
}
fn check_out<T>(p: *mut T) -> Result<()> {
    if p.is_null() || !(p as usize).is_multiple_of(std::mem::align_of::<T>()) {
        Err(AbiError::InvalidArgument)
    } else {
        Ok(())
    }
}

#[no_mangle]
pub extern "C" fn rl_opcoes_versao() -> u32 {
    VERSION
}

/// # Safety
/// `values` readable/aligned; `background.data` readable for 480000 bytes;
/// `output` writable/aligned.
#[no_mangle]
pub unsafe extern "C" fn rl_opcoes_criar(
    values_in: *const RlOpcoesValores,
    background: shell_abi::RlShellBytes,
    output: *mut u64,
) -> i32 {
    status(|| {
        let v = unsafe { read(values_in) }?;
        check_out(output)?;
        let (v, language) = values(&v)?;
        if background.data.is_null() || background.length as usize != PIXELS {
            return Err(AbiError::InvalidArgument);
        }
        let pixels = unsafe { slice::from_raw_parts(background.data, PIXELS) };
        let options = Options::new(v, language, pixels).map_err(render)?;
        let handle = with_registry(|r| {
            let h = r.next;
            r.next = h.checked_add(1).ok_or(AbiError::ResourceLimit)?;
            Ok(h)
        })?;
        registry(|r| {
            r.insert(handle, options);
            Ok(())
        })?;
        unsafe { write(output, handle) }
    })
}

/// # Safety
/// `values_in` readable/aligned.
#[no_mangle]
pub unsafe extern "C" fn rl_opcoes_definir(handle: u64, values_in: *const RlOpcoesValores) -> i32 {
    status(|| {
        let v = unsafe { read(values_in) }?;
        let (v, language) = values(&v)?;
        with_options(handle, |o| {
            o.set_values(v).map_err(render)?;
            o.language = language;
            Ok(())
        })
    })
}

/// # Safety
/// `output` writable/aligned.
#[no_mangle]
pub unsafe extern "C" fn rl_opcoes_valores(handle: u64, output: *mut RlOpcoesValores) -> i32 {
    status(|| {
        check_out(output)?;
        let v = registry(|r| Ok(c_values(r.get(&handle).ok_or(AbiError::InvalidHandle)?)))?;
        unsafe { write(output, v) }
    })
}

/// # Safety
/// `output` writable/aligned.
#[no_mangle]
pub unsafe extern "C" fn rl_opcoes_aberta(handle: u64, output: *mut u32) -> i32 {
    status(|| {
        check_out(output)?;
        let open = registry(|r| Ok(r.get(&handle).ok_or(AbiError::InvalidHandle)?.is_open()))?;
        unsafe { write(output, u32::from(open)) }
    })
}

#[no_mangle]
pub extern "C" fn rl_opcoes_abrir(handle: u64, shell: u64) -> i32 {
    status(|| {
        with_options(handle, |o| {
            with_look(shell, |look, pending| {
                if pending {
                    return Err(AbiError::InvalidArgument);
                }
                o.open(look).map_err(render)
            })
        })
    })
}

#[no_mangle]
pub extern "C" fn rl_opcoes_fechar(handle: u64) -> i32 {
    status(|| {
        with_options(handle, |o| {
            o.close();
            Ok(())
        })
    })
}

/// # Safety
/// `input` readable/aligned; `output` writable/aligned.
#[no_mangle]
pub unsafe extern "C" fn rl_opcoes_entrada(
    handle: u64,
    shell: u64,
    input: *const RlOpcoesEntrada,
    output: *mut RlOpcoesEvento,
) -> i32 {
    status(|| {
        let i = unsafe { read(input) }?;
        check_out(output)?;
        if i.version != VERSION
            || i.keys > 63
            || i.mouse_x > 0xffff
            || i.mouse_y > 0xffff
            || i.mouse_ae > 255
            || i.mouse_af > 255
            || i.reserved != 0
        {
            return Err(AbiError::InvalidArgument);
        }
        let input = Input {
            enter: i.keys & KEY_ENTER != 0,
            up: i.keys & KEY_UP != 0,
            down: i.keys & KEY_DOWN != 0,
            escape: i.keys & KEY_ESCAPE != 0,
            left: i.keys & KEY_LEFT != 0,
            right: i.keys & KEY_RIGHT != 0,
            x: i.mouse_x as i32,
            y: i.mouse_y as i32,
            held: i.mouse_ae != 0,
            released: i.mouse_af != 0,
            ticks: i.ticks,
        };
        let event = with_options(handle, |o| {
            let (consumed, event) = with_look(shell, |look, pending| {
                if pending && !o.is_open() {
                    // A pending shell request belongs to the host first.
                    return Ok((false, Event::None));
                }
                o.input(look, &input).map_err(render)
            })?;
            let (kind, item) = match event {
                Event::None => (0, NO_ITEM),
                Event::Opened => (1, NO_ITEM),
                Event::Changed(item) => (2, item as u32),
                Event::ConfigureKeys => (3, Item::ConfigureKeys as u32),
                Event::Closed => (4, NO_ITEM),
            };
            Ok(RlOpcoesEvento {
                kind,
                consumed: u32::from(consumed),
                item,
                open: u32::from(o.is_open()),
                values: c_values(o),
            })
        })?;
        unsafe { write(output, event) }
    })
}

/// # Safety
/// `pixels` readable and writable for `capacity` (>= 480000) bytes.
#[no_mangle]
pub unsafe extern "C" fn rl_opcoes_sobrepor(
    handle: u64,
    shell: u64,
    pixels: *mut u8,
    capacity: u32,
) -> i32 {
    status(|| {
        if pixels.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        if (capacity as usize) < PIXELS {
            return Err(AbiError::BufferTooSmall);
        }
        let target = unsafe { slice::from_raw_parts_mut(pixels, PIXELS) };
        let mut copy = target.to_vec();
        with_options(handle, |o| {
            with_look(shell, |look, _| o.overlay(look, &mut copy).map_err(render))
        })?;
        target.copy_from_slice(&copy);
        Ok(())
    })
}

/// # Safety
/// `pixels`/`palette` writable for their capacities (480000 / 768), disjoint.
#[no_mangle]
pub unsafe extern "C" fn rl_opcoes_desenhar(
    handle: u64,
    shell: u64,
    pixels: *mut u8,
    pixel_capacity: u32,
    palette: *mut u8,
    palette_capacity: u32,
) -> i32 {
    status(|| {
        if pixels.is_null() || palette.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        if (pixel_capacity as usize) < PIXELS || palette_capacity < 768 {
            return Err(AbiError::BufferTooSmall);
        }
        let (p, q) = (pixels as usize, palette as usize);
        if p < q + 768 && q < p + PIXELS {
            return Err(AbiError::InvalidArgument);
        }
        let mut picture = vec![0u8; PIXELS];
        let rgb = with_options(handle, |o| {
            with_look(shell, |look, _| {
                o.draw(look, &mut picture).map_err(render)?;
                Ok(look.fonts.palette)
            })
        })?;
        unsafe {
            ptr::copy_nonoverlapping(picture.as_ptr(), pixels, PIXELS);
            ptr::copy_nonoverlapping(rgb.as_ptr(), palette, 768);
        }
        Ok(())
    })
}

/// # Safety
/// `rect` writable/aligned for four i32 (left, top, right, bottom exclusive).
#[no_mangle]
pub unsafe extern "C" fn rl_opcoes_botao(handle: u64, shell: u64, rect: *mut i32) -> i32 {
    status(|| {
        check_out(rect)?;
        let r = registry(|r| {
            let o = r.get(&handle).ok_or(AbiError::InvalidHandle)?;
            with_look(shell, |look, _| o.button_rect(look).map_err(render))
        })?;
        unsafe { ptr::copy_nonoverlapping(r.as_ptr(), rect, 4) };
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn rl_opcoes_liberar(handle: u64) -> i32 {
    status(|| registry(|r| r.remove(&handle).map(|_| ()).ok_or(AbiError::InvalidHandle)))
}

#[cfg(test)]
#[path = "options_abi_tests.rs"]
mod options_abi_tests;
