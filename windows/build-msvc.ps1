<#
.SYNOPSIS
  Builds the Windows host with MSVC: the Rust engine as a static library for an
  *-pc-windows-msvc target, then the Win32 host compiled by cl.exe and linked
  against it into one RisingLands.exe (static CRT, system DLLs only).

.EXAMPLE
  windows\build-msvc.ps1                  # x64 -> windows\build\msvc-x64\RisingLands.exe
  windows\build-msvc.ps1 -Test            # also builds and runs the portable unit tests with cl.exe

.NOTES
  The Windows client is x86-64 only (decision of 2026-11-14; pre-alphas 1-3 also
  shipped an ARM64 build). Needs Visual Studio 2022 (or Build Tools) with the C++
  workload and rustup/cargo. It finds vcvarsall.bat with vswhere when cl.exe for
  x64 is not already on PATH. Tests run only on an x64 machine.
#>
param(
  [ValidateSet('x64')][string]$Arch = 'x64',
  [switch]$Test,
  [string]$Out = ''
)
$ErrorActionPreference = 'Stop'
$Here = $PSScriptRoot
$Repo = (Resolve-Path (Join-Path $Here '..')).Path
if (-not $Out) { $Out = Join-Path $Here "build\msvc-$Arch" }
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$Out = (Resolve-Path $Out).Path

function Invoke-Checked([string]$Exe, [string[]]$Arguments, [string]$WorkDir = '') {
  # cl.exe writes objects and .pdb to the working directory: running it inside
  # the object folder avoids /Fo"dir\" (a trailing backslash escapes the quote
  # PowerShell adds around paths with spaces).
  if ($WorkDir) { Push-Location $WorkDir }
  try {
    & $Exe @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Exe failed with exit code $LASTEXITCODE" }
  } finally { if ($WorkDir) { Pop-Location } }
}

# --- MSVC environment -------------------------------------------------------
$HostArch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'arm64' } else { 'x64' }
if (-not ((Get-Command cl.exe -ErrorAction SilentlyContinue) -and $env:VSCMD_ARG_TGT_ARCH -eq $Arch)) {
  $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
  if (-not (Test-Path $vswhere)) { throw 'vswhere.exe not found: install Visual Studio 2022 or the Build Tools' }
  $component = 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64'
  $vs = & $vswhere -latest -products * -requires $component -property installationPath
  if (-not $vs) { throw "No Visual Studio installation with $component" }
  $names = @{ x64 = 'amd64'; arm64 = 'arm64' }
  $vcarg = if ($Arch -eq $HostArch) { $names[$Arch] } else { "$($names[$HostArch])_$($names[$Arch])" }
  $vcvars = Join-Path $vs 'VC\Auxiliary\Build\vcvarsall.bat'
  Write-Host "== MSVC environment ($vcarg)"
  $envDump = cmd.exe /d /c "`"$vcvars`" $vcarg >nul && set"
  if ($LASTEXITCODE -ne 0) { throw "vcvarsall.bat $vcarg failed" }
  foreach ($line in $envDump) {
    $i = $line.IndexOf('=')
    if ($i -gt 0) { [Environment]::SetEnvironmentVariable($line.Substring(0, $i), $line.Substring($i + 1)) }
  }
}

# Same strictness as the gcc build (-Wall -Wextra, no -Wconversion): /W3, with the
# implicit-narrowing warnings C4244/C4267 off; the CRT "secure"/POSIX-name nags off.
$Common = @('/nologo', '/std:c11', '/utf-8', '/MT', '/W3', '/wd4244', '/wd4267',
  '/DUNICODE', '/D_UNICODE', '/D_CRT_SECURE_NO_WARNINGS', '/D_CRT_NONSTDC_NO_WARNINGS')
$Src = Join-Path $Here 'src'
$Inc = Join-Path $Repo 'engine\include'
# MacOSAudioMapper (portable C, reused unchanged by the Windows sound host).
$Mac = Join-Path $Repo 'macos\src'

# --- portable unit tests ----------------------------------------------------
if ($Test) {
  $T = Join-Path $Out 'tests'
  New-Item -ItemType Directory -Force -Path $T | Out-Null
  $TestLibs = @('shell32.lib', 'ole32.lib', 'uuid.lib', 'advapi32.lib')
  $suites = @(
    @{ Name = 'test_cd'; Defines = @('/DRLW_TESTING'); Files = @('tests\test_cd.c', 'src\rlw_cd.c', 'src\rlw_fs.c', 'src\rlw_sha256.c') },
    @{ Name = 'test_menu'; Defines = @(); Files = @('tests\test_menu.c', 'src\rlw_menu.c', 'src\rlw_flc.c', 'src\rlw_fs.c') },
    @{ Name = 'test_saves'; Defines = @(); Files = @('tests\test_saves.c', 'src\rlw_saves.c', 'src\rlw_fs.c') },
    @{ Name = 'test_savemgr'; Defines = @(); Files = @('tests\test_savemgr.c', 'src\rlw_saves.c', 'src\rlw_savemgr.c', 'src\rlw_autosave.c', 'src\rlw_fs.c') },
    @{ Name = 'test_gamemenu'; Defines = @(); Files = @('tests\test_gamemenu.c', 'src\rlw_gamemenu.c', 'src\rlw_saves.c', 'src\rlw_savemgr.c', 'src\rlw_autosave.c', 'src\rlw_fs.c') },
    @{ Name = 'test_hotkeys'; Defines = @(); Files = @('tests\test_hotkeys.c', 'src\rlw_hotkeys.c', 'src\rlw_modern.c') },
    @{ Name = 'test_markers'; Defines = @(); Files = @('tests\test_markers.c', 'src\rlw_markers.c') },
    @{ Name = 'test_audio'; Defines = @('/DRLW_TESTING', "/I$Mac"); Files = @('tests\test_audio.c', 'src\rlw_audio.c', 'src\rlw_sound.c', 'src\rlw_sound_import.c', 'src\rlw_cdmusic.c', 'src\rlw_eventflc.c', 'src\rlw_flc.c', 'src\rlw_movie.c', 'src\rlw_cd.c', 'src\rlw_fs.c', 'src\rlw_sha256.c', '..\macos\src\macos_audio_mapper.c') }
  )
  foreach ($s in $suites) {
    Write-Host "== $($s.Name) (cl $Arch)"
    $files = $s.Files | ForEach-Object { Join-Path $Here $_ }
    $obj = Join-Path $T "obj\$($s.Name)"
    New-Item -ItemType Directory -Force -Path $obj | Out-Null
    Invoke-Checked 'cl.exe' ($Common + @('/O1', '/Zi', '/WX', "/I$Src", "/I$Inc") + $s.Defines + $files + $TestLibs +
      @("/Fe$(Join-Path $T "$($s.Name).exe")")) $obj
  }
  if ($Arch -eq $HostArch) {
    Invoke-Checked (Join-Path $T 'test_cd.exe') @((Join-Path $T 'test-work'))
    Invoke-Checked (Join-Path $T 'test_menu.exe') @()
    Invoke-Checked (Join-Path $T 'test_saves.exe') @($T)
    Invoke-Checked (Join-Path $T 'test_savemgr.exe') @($T)
    Invoke-Checked (Join-Path $T 'test_gamemenu.exe') @($T)
    Invoke-Checked (Join-Path $T 'test_hotkeys.exe') @()
    Invoke-Checked (Join-Path $T 'test_markers.exe') @()
    Invoke-Checked (Join-Path $T 'test_audio.exe') @((Join-Path $T 'test-audio'))
  } else {
    Write-Host "== tests built for $Arch; not run on this $HostArch machine"
  }
}

# --- engine -----------------------------------------------------------------
$Triple = 'x86_64-pc-windows-msvc'
if (Get-Command rustup -ErrorAction SilentlyContinue) { Invoke-Checked 'rustup' @('target', 'add', $Triple) }
Write-Host "== engine ($Triple, static CRT)"
$env:CARGO_TARGET_DIR = Join-Path $Out 'engine-target'
# crt-static on every crate: the engine links libcmt like the host's /MT, so the
# .exe needs no vcruntime/ucrt redistributable.
[Environment]::SetEnvironmentVariable("CARGO_TARGET_$($Triple.ToUpper().Replace('-', '_'))_RUSTFLAGS", '-C target-feature=+crt-static')
Invoke-Checked 'cargo' @('rustc', '--release', '--locked', '--manifest-path', (Join-Path $Repo 'engine\Cargo.toml'),
  '-p', 'rl-ffi', '--lib', '--crate-type', 'staticlib', '--target', $Triple)
$Lib = Join-Path $env:CARGO_TARGET_DIR "$Triple\release\rising_engine.lib"
if (-not (Test-Path $Lib)) { throw "missing $Lib" }

# --- host -------------------------------------------------------------------
Write-Host "== host (cl $Arch)"
$obj = Join-Path $Out 'obj'
New-Item -ItemType Directory -Force -Path $obj | Out-Null
$Sources = @('rlw_main.c', 'rlw_driver.c', 'rlw_cd.c', 'rlw_fs.c', 'rlw_sha256.c', 'rlw_menu.c', 'rlw_menu_ops.c',
  'rlw_menu_demo.c', 'rlw_flc.c', 'rlw_saves.c', 'rlw_hotkeys.c', 'rlw_modern.c', 'rlw_keys_dialog.c',
  'rlw_markers.c', 'rlw_markers_gdi.c', 'rlw_smoke.c', 'rlw_dpi.c', 'rlw_savemgr.c', 'rlw_savemgr_dialog.c', 'rlw_autosave.c', 'rlw_gamemenu.c', 'rlw_gamemenu_win.c',
  'rlw_audio.c', 'rlw_audio_win32.c', 'rlw_audio_ui.c', 'rlw_sound.c', 'rlw_sound_import.c', 'rlw_cdmusic.c', 'rlw_eventflc.c', 'rlw_movie.c') | ForEach-Object { Join-Path $Src $_ }
$Sources += Join-Path $Mac 'macos_audio_mapper.c'
# Win32 libraries of the host, then what the Rust std needs on windows-msvc
# (rustc --print native-static-libs).
$Libs = @('user32.lib', 'gdi32.lib', 'comdlg32.lib', 'comctl32.lib', 'shell32.lib', 'ole32.lib', 'uuid.lib', 'winmm.lib',
  'kernel32.lib', 'advapi32.lib', 'ntdll.lib', 'userenv.lib', 'ws2_32.lib', 'dbghelp.lib', 'bcrypt.lib')
$Exe = Join-Path $Out 'RisingLands.exe'
Invoke-Checked 'cl.exe' ($Common + @('/O2', "/I$Inc", "/I$Src", "/I$Mac") + $Sources + @($Lib) + $Libs +
  @("/Fe$Exe", '/link', '/SUBSYSTEM:WINDOWS', '/INCREMENTAL:NO')) $obj

# One .exe with system DLLs only: no vcruntime/msvcp/ucrt redistributable.
$deps = & dumpbin.exe /nologo /dependents $Exe | Where-Object { $_ -match '^\s+\S+\.dll\s*$' } | ForEach-Object { $_.Trim() }
Write-Host "== imports: $($deps -join ' ')"
$bad = @($deps | Where-Object { $_ -match '^(vcruntime|msvcp|ucrtbase|api-ms-win-crt)' })
if ($bad.Count) { throw "non-system DLL imports: $($bad -join ' ')" }
Write-Host "== $Exe"
