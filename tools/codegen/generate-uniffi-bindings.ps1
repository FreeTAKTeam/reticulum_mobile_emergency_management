param(
  [ValidateSet("swift", "kotlin")]
  [string]$Language = "swift",
  [string]$OutDir = ""
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$udlPath = Join-Path $repoRoot "crates\reticulum_mobile\src\reticulum_mobile.udl"
$tempDir = Join-Path $repoRoot "target\uniffi\$Language"

if (-not $OutDir) {
  if ($Language -eq "kotlin") {
    $OutDir = "apps/mobile/android/uniffi"
  } else {
    $OutDir = "apps/mobile/ios/uniffi"
  }
}

$targetOut = Join-Path $repoRoot $OutDir
New-Item -ItemType Directory -Force -Path $tempDir | Out-Null
New-Item -ItemType Directory -Force -Path $targetOut | Out-Null

if ($Language -eq "kotlin") {
  $sdkRoot = if ($env:ANDROID_SDK_ROOT) { $env:ANDROID_SDK_ROOT } elseif ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { Join-Path $env:LOCALAPPDATA "Android\Sdk" }
  if (-not (Test-Path $sdkRoot)) {
    throw "Android SDK not found. Expected at $sdkRoot"
  }

  $ndkRoot = Join-Path $sdkRoot "ndk"
  if (-not (Test-Path $ndkRoot)) {
    throw "Android NDK not found. Install with sdkmanager first. Missing: $ndkRoot"
  }

  $ndkPath = $null
  if ($env:ANDROID_NDK_HOME -and (Test-Path $env:ANDROID_NDK_HOME)) {
    $ndkPath = $env:ANDROID_NDK_HOME
  } elseif ($env:NDK_HOME -and (Test-Path $env:NDK_HOME)) {
    $ndkPath = $env:NDK_HOME
  } else {
    $localProps = Join-Path $repoRoot "apps\mobile\android\local.properties"
    if (Test-Path $localProps) {
      $ndkLine = Get-Content $localProps | Where-Object { $_ -match "^ndk\.dir=" } | Select-Object -First 1
      if ($ndkLine) {
        $candidate = ($ndkLine -replace "^ndk\.dir=", "") -replace "\\\\", "\"
        if (Test-Path $candidate) {
          $ndkPath = $candidate
        }
      }
    }
  }

  if (-not $ndkPath) {
    $latestNdkDir = Get-ChildItem $ndkRoot -Directory | Sort-Object Name -Descending | Select-Object -First 1
    if (-not $latestNdkDir) {
      throw "No Android NDK versions found under $ndkRoot"
    }
    $ndkPath = $latestNdkDir.FullName
  }

  $ndkBin = Join-Path $ndkPath "toolchains\llvm\prebuilt\windows-x86_64\bin"
  if (-not (Test-Path $ndkBin)) {
    throw "NDK LLVM toolchain bin not found: $ndkBin"
  }

  $env:ANDROID_HOME = $sdkRoot
  $env:ANDROID_SDK_ROOT = $sdkRoot
  $env:ANDROID_NDK_HOME = $ndkPath
  $env:NDK_HOME = $ndkPath
  $env:PATH = "$ndkBin;" + (Join-Path $sdkRoot "platform-tools") + ";" + (Join-Path $sdkRoot "cmdline-tools\latest\bin") + ";" + $env:PATH

  # Explicit linker and C compiler mapping for cc-rs/ring when cross-compiling.
  $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = Join-Path $ndkBin "aarch64-linux-android21-clang.cmd"
  $env:CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER = Join-Path $ndkBin "armv7a-linux-androideabi21-clang.cmd"
  $env:CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER = Join-Path $ndkBin "x86_64-linux-android21-clang.cmd"
  $env:CC_aarch64_linux_android = Join-Path $ndkBin "aarch64-linux-android21-clang.cmd"
  $env:CC_armv7_linux_androideabi = Join-Path $ndkBin "armv7a-linux-androideabi21-clang.cmd"
  $env:CC_x86_64_linux_android = Join-Path $ndkBin "x86_64-linux-android21-clang.cmd"

  Write-Host "Using Android SDK: $sdkRoot"
  Write-Host "Using Android NDK: $ndkPath"
}

if ($Language -eq "kotlin") {
  $targets = @(
    "aarch64-linux-android",
    "armv7-linux-androideabi",
    "x86_64-linux-android"
  )
} else {
  $targets = @(
    "aarch64-apple-ios",
    "aarch64-apple-ios-sim",
    "x86_64-apple-ios-sim"
  )
}

Write-Host "Building reticulum_mobile for $Language targets..."
foreach ($target in $targets) {
  rustup target add $target | Out-Null
  if ($LASTEXITCODE -ne 0) {
    throw "rustup target add failed for $target"
  }
  cargo build -p reticulum_mobile --release --target $target
  if ($LASTEXITCODE -ne 0) {
    throw "cargo build failed for $target"
  }
}

$bindgen = Get-Command uniffi-bindgen -ErrorAction SilentlyContinue
if ($bindgen) {
  Write-Host "Generating UniFFI bindings ($Language)..."
  uniffi-bindgen generate $udlPath --language $Language --out-dir $tempDir
  if ($LASTEXITCODE -ne 0) {
    throw "uniffi-bindgen generation failed for $Language"
  }

  Write-Host "Copying generated bindings to $targetOut..."
  Copy-Item -Force -Recurse (Join-Path $tempDir "*") $targetOut
} elseif ($Language -eq "kotlin") {
  Write-Warning "uniffi-bindgen not found; skipping Kotlin binding generation. Native .so files will still be copied."
} else {
  throw "uniffi-bindgen not found in PATH"
}

Write-Host "Copying built native libraries..."
foreach ($target in $targets) {
  $releaseDir = Join-Path $repoRoot "target\$target\release"
  if (-not (Test-Path $releaseDir)) {
    continue
  }
  $targetLibDir = Join-Path $targetOut "libs\$target"
  New-Item -ItemType Directory -Force -Path $targetLibDir | Out-Null
  Get-ChildItem $releaseDir -File -Filter "libreticulum_mobile*" |
    ForEach-Object { Copy-Item -Force $_.FullName $targetLibDir }

  if ($Language -eq "kotlin") {
    $abi = switch ($target) {
      "aarch64-linux-android" { "arm64-v8a" }
      "armv7-linux-androideabi" { "armeabi-v7a" }
      "x86_64-linux-android" { "x86_64" }
      default { "" }
    }
    if ($abi -ne "") {
      $jniLibDir = Join-Path $repoRoot "apps\mobile\android\app\src\main\jniLibs\$abi"
      New-Item -ItemType Directory -Force -Path $jniLibDir | Out-Null
      Get-ChildItem $releaseDir -File -Filter "libreticulum_mobile.so" |
        ForEach-Object { Copy-Item -Force $_.FullName $jniLibDir }
    }
  }
}

Write-Host "UniFFI generation complete."
