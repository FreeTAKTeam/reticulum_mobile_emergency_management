param(
  [string]$Language = "swift",
  [string]$OutDir = "apps/mobile/ios"
)

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$udl = Join-Path $repoRoot "crates\reticulum_mobile\src\reticulum_mobile.udl"
$targetOut = Join-Path $repoRoot $OutDir

uniffi-bindgen generate $udl --language $Language --out-dir $targetOut
