# Install MSVC Build Tools unattended.
#
# Strategy: this script does only the prep (download bootstrapper), then
# re-launches the bootstrapper itself elevated. The parent (non-elevated)
# PowerShell waits for the elevated child to exit.
#
# Why not re-launch the .ps1 elevated? UAC-elevated PowerShell windows don't
# share stdout with the parent, so we lose the installer's progress. By
# elevating the .exe directly we can keep -Wait and -RedirectStandardOutput.

$ErrorActionPreference = "Stop"

$bootstrapperUrl = "https://aka.ms/vs/17/release/vs_buildtools.exe"
$bootstrapper    = Join-Path $env:TEMP "vs_buildtools.exe"
$installLog      = Join-Path $env:TEMP "vs_buildtools_install.log"

$components = @(
    "Microsoft.VisualStudio.Workload.VCTools",
    "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
    "Microsoft.VisualStudio.Component.VC.ATL",
    # Windows Universal C Runtime (the SDK base) — required by VCTools.
    "Microsoft.VisualStudio.Component.Windows10SDK",
    # Recommended full SDK; provides kernel32.lib, ucrt.lib, etc.
    "Microsoft.VisualStudio.Component.Windows11SDK.26100"
)

# Download bootstrapper (works without admin).
if (-not (Test-Path $bootstrapper)) {
    Write-Host "Downloading VS Build Tools bootstrapper..."
    Invoke-WebRequest -Uri $bootstrapperUrl -OutFile $bootstrapper -UseBasicParsing
}
Write-Host "Bootstrapper: $bootstrapper"

# Build argument list. Quote component names with spaces.
$argList = @(
    "--quiet"
    "--wait"
    "--norestart"
    "--nocache"
    "--installPath", "C:\BuildTools"
)
foreach ($c in $components) {
    $argList += @("--add", $c)
}

Write-Host "Launching elevated installer (logs auto-written to %TEMP%\dd_*.log)..."

# Run the bootstrapper elevated. -Verb RunAs triggers a UAC prompt.
# -Wait blocks until it finishes. -PassThru gives us exit code.
# Note: cannot combine -Verb RunAs with -RedirectStandardOutput, and the
# inner setup.exe doesn't accept --log anyway (its dd_*.log files in %TEMP%
# are written automatically).
$proc = Start-Process -FilePath $bootstrapper `
                      -ArgumentList $argList `
                      -Verb RunAs `
                      -Wait `
                      -PassThru

Write-Host "Installer exit code: $($proc.ExitCode)"

$vcvars = "C:\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
if (Test-Path $vcvars) {
    Write-Host ""
    Write-Host "MSVC install verified. vcvars64.bat: $vcvars"
    Write-Host "Next: open a new PowerShell, run:"
    Write-Host "  `"$vcvars`""
    Write-Host "Then:"
    Write-Host "  cargo install tauri-cli --version '^2' --locked"
    Write-Host "  cd `"D:\Coding\Claude\Whatsapp Rust`""
    Write-Host "  cargo tauri build"
} else {
    Write-Host ""
    Write-Host "WARNING: vcvars64.bat not found at $vcvars. Install may have failed."
    Write-Host "Check $installLog and $installLog.err."
}