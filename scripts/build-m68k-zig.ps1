<#
.SYNOPSIS
    build a Zig 0.16.0 toolchain with the experimental llvm m68k backend

.DESCRIPTION
      1. build llvm 21.x + clang + lld from source
      2. build Zig 0.16.0 from source against that llvm

    notes:
      - run from a PowerShell prompt
      - zig 0.16.0 should be built against llvm 21 using release/21.x branch
      - llvm m68k flag: -DLLVM_EXPERIMENTAL_TARGETS_TO_BUILD=M68k
      - zig m68k flag: -Dllvm-has-m68k

.EXAMPLE
    pwsh -File scripts\build-m68k-zig.ps1

.EXAMPLE
    pwsh -File scripts\build-m68k-zig.ps1 -WorkDir D:\m68k -LlvmPrefix D:\llvm21-m68k -ZigPrefix D:\zig-m68k
#>
[CmdletBinding()]
param(
    [string]$WorkDir    = "D:\m68k-build",
    [string]$LlvmPrefix = "D:\llvm21-m68k",
    [string]$ZigPrefix  = "D:\zig-m68k",
    [string]$LlvmBranch = "release/21.x",
    [string]$ZigVersion = "0.16.0",
    [string]$ZigSrcSha256 = "43186959edc87d5c7a1be7b7d2a25efffd22ce5807c7af99067f86f99641bfdf",
    # Parallel link jobs. LLVM links are memory-hungry (several GB each); 2 is safe on 32 GB.
    [int]$LinkJobs     = 2,
    # The existing Zig used to bootstrap the build (must match $ZigVersion).
    [string]$BootstrapZig = "zig"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Invoke-Native {
    param([Parameter(Mandatory)][string]$Exe, [string[]]$Arguments = @())
    Write-Host ">> $Exe $($Arguments -join ' ')" -ForegroundColor Cyan
    & $Exe @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Exe exited with code $LASTEXITCODE" }
}

Write-Host "== Locating Visual Studio (vswhere) ==" -ForegroundColor Green
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path $vswhere)) { throw "vswhere not found; install Visual Studio Build Tools with the C++ workload." }
$vsPath = (& $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath | Select-Object -First 1)
if (-not $vsPath) { throw "No Visual Studio install with the C++ (VC.Tools.x86.x64) component was found." }
Write-Host "Visual Studio: $vsPath"

$devShellDll = Join-Path $vsPath "Common7\Tools\Microsoft.VisualStudio.DevShell.dll"
Import-Module $devShellDll
Enter-VsDevShell -VsInstallPath $vsPath -SkipAutomaticLocation -DevCmdArguments "-arch=x64 -host_arch=x64" | Out-Null

$cmakeBin = Join-Path $vsPath "Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin"
$ninjaBin = Join-Path $vsPath "Common7\IDE\CommonExtensions\Microsoft\CMake\Ninja"
$env:PATH = "$cmakeBin;$ninjaBin;$env:PATH"

foreach ($t in @("cl", "cmake", "ninja", "git")) {
    $c = Get-Command $t -ErrorAction SilentlyContinue
    if (-not $c) { throw "Required tool '$t' not found on PATH after entering the VS dev shell." }
    Write-Host ("  {0,-6} {1}" -f $t, $c.Source)
}
$null = Get-Command $BootstrapZig -ErrorAction Stop

New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null

$llvmSrc   = Join-Path $WorkDir "llvm-project"
$llvmBuild = Join-Path $WorkDir "llvm-build"

if (-not (Test-Path (Join-Path $llvmSrc "llvm\CMakeLists.txt"))) {
    Write-Host "== Cloning llvm-project ($LlvmBranch, shallow) ==" -ForegroundColor Green
    Invoke-Native git @("clone", "--depth", "1", "--branch", $LlvmBranch,
        "https://github.com/llvm/llvm-project.git", $llvmSrc)
} else {
    Write-Host "== Reusing existing llvm-project checkout at $llvmSrc ==" -ForegroundColor Yellow
}

if (-not (Test-Path (Join-Path $LlvmPrefix "lib\LLVMM68kCodeGen.lib")) -and
    -not (Test-Path (Join-Path $LlvmPrefix "lib\libLLVMM68kCodeGen.a"))) {
    Write-Host "== Configuring LLVM ==" -ForegroundColor Green
    $cmakeArgs = @(
        "-G", "Ninja",
        "-S", (Join-Path $llvmSrc "llvm"),
        "-B", $llvmBuild,
        "-DCMAKE_BUILD_TYPE=Release",
        "-DCMAKE_INSTALL_PREFIX=$LlvmPrefix",
        "-DLLVM_ENABLE_PROJECTS=lld;clang",
        "-DLLVM_EXPERIMENTAL_TARGETS_TO_BUILD=M68k",
        "-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded",
        "-DLLVM_ENABLE_DIA_SDK=OFF",
        "-DLLVM_ENABLE_LIBXML2=OFF",
        "-DLLVM_ENABLE_ZLIB=OFF",
        "-DLLVM_ENABLE_ZSTD=OFF",
        "-DLLVM_INCLUDE_BENCHMARKS=OFF",
        "-DLLVM_INCLUDE_EXAMPLES=OFF",
        "-DLLVM_INCLUDE_TESTS=OFF",
        "-DLLVM_PARALLEL_LINK_JOBS=$LinkJobs"
    )
    Invoke-Native cmake $cmakeArgs

    Write-Host "== Building + installing LLVM (this is the long part) ==" -ForegroundColor Green
    Invoke-Native cmake @("--build", $llvmBuild, "--target", "install")
} else {
    Write-Host "== LLVM with M68k already installed at $LlvmPrefix; skipping ==" -ForegroundColor Yellow
}

$llvmLib = Join-Path $LlvmPrefix "bin\llvm-lib.exe"
foreach ($stub in @("z.lib", "zstd.lib")) {
    $stubPath = Join-Path $LlvmPrefix ("lib\" + $stub)
    if (-not (Test-Path $stubPath)) {
        Invoke-Native $llvmLib @("/OUT:$stubPath", "/llvmlibempty", "/ignore:emptyoutput")
    }
}

$zigTar = Join-Path $WorkDir "zig-$ZigVersion.tar.xz"
$zigSrc = Join-Path $WorkDir "zig-$ZigVersion"

if (-not (Test-Path (Join-Path $zigSrc "build.zig"))) {
    if (-not (Test-Path $zigTar)) {
        Write-Host "== Downloading Zig $ZigVersion source ==" -ForegroundColor Green
        Invoke-WebRequest -Uri "https://ziglang.org/download/$ZigVersion/zig-$ZigVersion.tar.xz" -OutFile $zigTar
    }
    $sha = (Get-FileHash $zigTar -Algorithm SHA256).Hash.ToLower()
    if ($sha -ne $ZigSrcSha256.ToLower()) {
        throw "Zig source checksum mismatch.`n  expected $ZigSrcSha256`n  got      $sha"
    }
    Write-Host "== Extracting Zig source ==" -ForegroundColor Green
    Invoke-Native tar @("-xf", $zigTar, "-C", $WorkDir)
}

Write-Host "== Building Zig with -Dllvm-has-m68k ==" -ForegroundColor Green
Push-Location $zigSrc
try {
    $zigArgs = @(
        "build",
        "-p", $ZigPrefix,
        "--search-prefix", $LlvmPrefix,
        "--zig-lib-dir", "lib",
        "-Dstatic-llvm",
        "-Dllvm-has-m68k",
        "-Dtarget=x86_64-windows-msvc",
        "-Doptimize=ReleaseFast"
    )
    Write-Host ">> $BootstrapZig $($zigArgs -join ' ')" -ForegroundColor Cyan
    & $BootstrapZig @zigArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Host "zig build exited $LASTEXITCODE (likely the max_rss soft-limit); verifying via the smoke test..." -ForegroundColor Yellow
    }
} finally {
    Pop-Location
}

$newZig = Join-Path $ZigPrefix "bin\zig.exe"
Write-Host "== Smoke test: $newZig ==" -ForegroundColor Green
Invoke-Native $newZig @("version")
$probe = Join-Path $WorkDir "probe.zig"
Set-Content -Path $probe -Value "export fn _t() callconv(.c) u32 { return 42; }" -Encoding utf8
Invoke-Native $newZig @("build-obj", "-target", "m68k-freestanding", "-femit-bin=$WorkDir\probe.o", $probe)

Write-Host ""
Write-Host "SUCCESS." -ForegroundColor Green
Write-Host "M68k-enabled Zig: $newZig"
Write-Host "Build the cartridge with:  & '$newZig' build rom"
Write-Host "Or put '$ZigPrefix\bin' first on PATH to make it the default zig."
