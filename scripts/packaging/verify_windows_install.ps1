# Run only on disposable release CI runners. Do not launch the editor.
param([Parameter(Mandatory=$true)][string]$Installer,
      [Parameter(Mandatory=$true)][string]$Binary)
$ErrorActionPreference = 'Stop'
$Installer = (Resolve-Path $Installer).Path
$CodecWorker = Join-Path (Split-Path (Resolve-Path $Binary).Path -Parent) 'paintcraft-codec-worker.exe'
$script:HelperFiles = @()
function Invoke-Msi([string[]]$MsiArguments) {
    $process = Start-Process msiexec.exe -ArgumentList $MsiArguments -Wait -PassThru
    if ($process.ExitCode -notin @(0, 3010)) {
        throw "Windows Installer failed with exit code $($process.ExitCode)"
    }
}
function Verify-Installed {
    $command = (Get-Item 'HKLM:\Software\Classes\PaintCraft.Document\shell\open\command').GetValue('')
    if ($command -notmatch '^"([^"]+paintcraft\.exe)" "%1"$') {
        throw "Incorrect .pcnd open command: $command"
    }
    $installed = $Matches[1]
    if ((Get-FileHash $installed -Algorithm SHA256).Hash -ne (Get-FileHash $Binary -Algorithm SHA256).Hash) {
        throw 'Installed executable differs from the release binary'
    }
    $directory = Split-Path $installed -Parent
    $helper = Join-Path $directory 'paintcraft-codec-worker.exe'
    $manifestPath = Join-Path $directory 'paintcraft-codec-worker.json'
    $script:HelperFiles = @($helper, $manifestPath)
    if ((Get-FileHash $helper -Algorithm SHA256).Hash -ne (Get-FileHash $CodecWorker -Algorithm SHA256).Hash) {
        throw 'Installed decoder helper differs from the release helper'
    }
    $manifest = Get-Content $manifestPath -Raw | ConvertFrom-Json
    if ($manifest.schema_version -ne 1 -or $manifest.protocol_version -ne 1 -or
        $manifest.executable -ne 'paintcraft-codec-worker.exe' -or $manifest.public_reader_enabled -ne $false -or
        $manifest.sha256 -ne (Get-FileHash $helper -Algorithm SHA256).Hash) {
        throw 'Installed helper manifest does not match executable/protocol'
    }
    $progId = (Get-Item 'HKLM:\Software\Classes\.pcnd').GetValue('')
    if ($progId -ne 'PaintCraft.Document') { throw 'Missing .pcnd ProgID' }
    $openWith = Get-Item 'HKLM:\Software\Classes\.pcnd\OpenWithProgids'
    if ('PaintCraft.Document' -notin $openWith.GetValueNames()) { throw 'Missing Open With registration' }
    $capability = (Get-Item 'HKLM:\Software\Replicraft\PaintCraft\Capabilities\FileAssociations').GetValue('.pcnd')
    if ($capability -ne 'PaintCraft.Document') { throw 'Missing Default Apps capability' }
    $registered = (Get-Item 'HKLM:\Software\RegisteredApplications').GetValue('PaintCraft')
    if ($registered -ne 'Software\Replicraft\PaintCraft\Capabilities') { throw 'Missing RegisteredApplications entry' }
    if ((Get-Item $installed).VersionInfo.ProductName -ne 'PaintCraft') { throw 'Missing executable metadata' }
}
$didInstall = $false
try {
    Invoke-Msi -MsiArguments @('/i', "`"$Installer`"", '/qn', '/norestart', '/l*v', 'msi-install.log')
    $didInstall = $true
    Verify-Installed
    Invoke-Msi -MsiArguments @('/fa', "`"$Installer`"", '/qn', '/norestart', '/l*v', 'msi-repair.log')
    Verify-Installed
} finally {
    if ($didInstall) {
        Invoke-Msi -MsiArguments @('/x', "`"$Installer`"", '/qn', '/norestart', '/l*v', 'msi-uninstall.log')
    }
}
if (Test-Path 'HKLM:\Software\Classes\PaintCraft.Document') { throw 'Uninstall left the owned ProgID' }
$registered = Get-Item 'HKLM:\Software\RegisteredApplications' -ErrorAction SilentlyContinue
if ($registered -and 'PaintCraft' -in $registered.GetValueNames()) { throw 'Uninstall left RegisteredApplications' }

foreach ($helperFile in $script:HelperFiles) {
    if (Test-Path $helperFile) { throw "Uninstall left decoder helper payload: $helperFile" }
}
