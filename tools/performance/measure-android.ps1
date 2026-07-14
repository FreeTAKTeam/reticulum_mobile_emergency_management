param(
  [string[]]$Serial,
  [string]$Package = "network.reticulum.emergency",
  [string]$Activity = "network.reticulum.emergency/.MainActivity",
  [ValidateRange(1, 20)]
  [int]$Samples = 3
)

$ErrorActionPreference = "Stop"

if (-not (Get-Command adb -ErrorAction SilentlyContinue)) {
  throw "adb was not found on PATH"
}

if (-not $Serial) {
  $Serial = @(
    adb devices |
      Select-Object -Skip 1 |
      Where-Object { $_ -match "\tdevice$" } |
      ForEach-Object { ($_ -split "\s+")[0] }
  )
}

if (-not $Serial) {
  throw "No authorized Android devices are connected"
}

function Get-LaunchTimeMs {
  param([string]$DeviceSerial)

  foreach ($attempt in 1..2) {
    $output = @(adb -s $DeviceSerial shell am start -W -n $Activity)
    $text = $output -join "`n"
    if ($text -match "(?m)^(?:TotalTime|WaitTime):\s*(\d+)\s*$") {
      return [int]$Matches[1]
    }
    Start-Sleep -Milliseconds 200
  }
  throw "adb did not report launch timing for $DeviceSerial`n$text"
}

$results = foreach ($deviceSerial in $Serial) {
  $state = (adb -s $deviceSerial get-state 2>$null).Trim()
  if ($state -ne "device") {
    throw "$deviceSerial is not ready (state: $state)"
  }

  adb -s $deviceSerial logcat -c | Out-Null
  $coldMs = @(
    foreach ($sample in 1..$Samples) {
      adb -s $deviceSerial shell am force-stop $Package | Out-Null
      Get-LaunchTimeMs -DeviceSerial $deviceSerial
    }
  )
  $warmMs = @(
    foreach ($sample in 1..$Samples) {
      adb -s $deviceSerial shell input keyevent KEYCODE_HOME | Out-Null
      Get-LaunchTimeMs -DeviceSerial $deviceSerial
    }
  )

  Start-Sleep -Seconds 2
  $memory = @(adb -s $deviceSerial shell dumpsys meminfo $Package)
  $graphics = @(adb -s $deviceSerial shell dumpsys gfxinfo $Package)
  $logs = @(adb -s $deviceSerial logcat -d -v brief)
  $manufacturer = ((adb -s $deviceSerial shell getprop ro.product.manufacturer) -join "").Trim()
  $model = ((adb -s $deviceSerial shell getprop ro.product.model) -join "").Trim()
  $android = ((adb -s $deviceSerial shell getprop ro.build.version.release) -join "").Trim()
  $sdk = ((adb -s $deviceSerial shell getprop ro.build.version.sdk) -join "").Trim()

  [pscustomobject]@{
    serial = $deviceSerial
    manufacturer = $manufacturer
    model = $model
    android = $android
    sdk = [int]$sdk
    coldLaunchMs = $coldMs
    coldAverageMs = [math]::Round((($coldMs | Measure-Object -Average).Average), 1)
    warmLaunchMs = $warmMs
    warmAverageMs = [math]::Round((($warmMs | Measure-Object -Average).Average), 1)
    memorySummary = (($memory | Select-String "^\s*TOTAL PSS:|^\s*TOTAL\s+" | Select-Object -Last 1).Line.Trim())
    frames = (($graphics | Select-String "Total frames rendered:" | Select-Object -First 1).Line.Trim())
    jankyFrames = (($graphics | Select-String "Janky frames:" | Select-Object -First 1).Line.Trim())
    criticalLogCount = @(
      $logs | Select-String "FATAL EXCEPTION|ANR in $Package|UnsatisfiedLinkError|JNI DETECTED ERROR"
    ).Count
  }
}

$results | ConvertTo-Json -Depth 4
