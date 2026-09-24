# Installs the `folderskin` command line on Windows, from its newest release (a cli-v* tag; the
# app's releases are separate), after checking the download against its SHA-256.
#
#   irm https://raw.githubusercontent.com/prajwal-svm/folderskin/main/scripts/install-cli.ps1 | iex
#   .\install-cli.ps1 -Version 0.1.0
#   .\install-cli.ps1 -InstallDir D:\tools\folderskin
#
# It goes in %LOCALAPPDATA%\Programs\folderskin and on your user PATH; nothing needs admin rights.
param(
    [string]$Version = $env:FOLDERSKIN_VERSION,
    [string]$InstallDir = $(if ($env:FOLDERSKIN_INSTALL_DIR) { $env:FOLDERSKIN_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\folderskin' })
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue' # the progress bar makes Invoke-WebRequest many times slower
$repo = 'prajwal-svm/folderskin'

function Say([string]$message) { Write-Host "folderskin: $message" }
function Fail([string]$what, [string]$try) {
    Write-Host "folderskin: $what" -ForegroundColor Red
    if ($try) { Write-Host "  Try: $try" }
    $arch = $env:PROCESSOR_ARCHITECTURE
    # Safe to paste back into PowerShell: no quote of any kind inside the double quotes (it takes
    # typographic quotes, U+2018 to U+201F, for plain ones), no $ and no backtick.
    $plain = $what -replace ('["`' + [char]0x2018 + '-' + [char]0x201F + ']'), "'" -replace '[$]', 'USD '
    Write-Host ('  Or ask Claude: claude "The folderskin installer failed on Windows ' + $arch + ': ' + $plain + ' Help me install it."')
    # Not `exit`: run through `iex`, that would close the PowerShell window it was typed into.
    throw 'folderskin: the install stopped; nothing was changed.'
}

if (-not [Environment]::Is64BitOperatingSystem) {
    Fail 'folderskin needs 64-bit Windows.' 'Use a 64-bit Windows 10 or 11.'
}
if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') {
    Say 'this is Windows on ARM: the x86_64 build runs under Windows'' own emulation'
}

# Newer TLS for older PowerShell 5.1 set-ups.
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

if ($Version) {
    $tag = 'cli-v' + ($Version -replace '^v', '')
} else {
    try {
        # The app's releases share the repository, so the newest command line is the first cli-v tag.
        $releases = Invoke-RestMethod -UseBasicParsing -Uri "https://api.github.com/repos/$repo/releases?per_page=100"
        $tag = ($releases | Where-Object { $_.tag_name -like 'cli-v*' -and -not $_.draft } | Select-Object -First 1).tag_name
    } catch {
        $tag = $null
    }
    if (-not $tag) {
        Fail "Couldn't find a folderskin release on GitHub." 'Check the internet connection, or name a version: .\install-cli.ps1 -Version 0.1.0'
    }
}
$v = $tag -replace '^cli-v', ''
$asset = "folderskin-cli-$v-windows-x86_64.zip"
$base = "https://github.com/$repo/releases/download/$tag"
$tmp = Join-Path ([IO.Path]::GetTempPath()) ("folderskin-install-" + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null

try {
    Say "downloading $asset"
    try {
        Invoke-WebRequest -UseBasicParsing -Uri "$base/$asset" -OutFile (Join-Path $tmp $asset)
        Invoke-WebRequest -UseBasicParsing -Uri "$base/$asset.sha256" -OutFile (Join-Path $tmp "$asset.sha256")
    } catch {
        Fail "Couldn't download $asset from $base." "Check the version exists at https://github.com/$repo/releases, then run this again."
    }
    $want = ((Get-Content (Join-Path $tmp "$asset.sha256") -Raw).Trim() -split '\s+')[0].ToLowerInvariant()
    $have = (Get-FileHash -Algorithm SHA256 -Path (Join-Path $tmp $asset)).Hash.ToLowerInvariant()
    if ($want -ne $have) {
        Fail "The download doesn't match its published SHA-256, so nothing was installed." "Run this again; if it happens twice, report it at https://github.com/$repo/issues"
    }
    Expand-Archive -Path (Join-Path $tmp $asset) -DestinationPath (Join-Path $tmp 'x') -Force

    try {
        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
        Copy-Item -Force (Join-Path $tmp 'x\folderskin.exe') (Join-Path $InstallDir 'folderskin.exe')
    } catch {
        Fail "Couldn't write $InstallDir\folderskin.exe: $($_.Exception.Message)" 'Close any terminal running folderskin, or pick another folder with -InstallDir.'
    }
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

# On the user PATH from the next terminal on, and in this one now. The PATH is read and written
# as it is stored: [Environment]::GetEnvironmentVariable hands it back with every %VARIABLE%
# already expanded, and writing that back would pin entries like %USERPROFILE%\... for good.
$envKey = 'HKCU:\Environment'
if (-not (Test-Path $envKey)) { New-Item -Path $envKey | Out-Null }
$userPath = (Get-Item $envKey).GetValue('Path', '', 'DoNotExpandEnvironmentNames')
$parts = @($userPath -split ';' | Where-Object { $_ })
$already = $parts | Where-Object { [Environment]::ExpandEnvironmentVariables($_).TrimEnd('\') -eq $InstallDir.TrimEnd('\') }
if (-not $already) {
    Set-ItemProperty -Path $envKey -Name 'Path' -Type ExpandString -Value (($parts + $InstallDir) -join ';')
    # Setting a variable through .NET tells Windows the environment changed (WM_SETTINGCHANGE),
    # so Explorer, and terminals opened from it, see the new PATH without signing out.
    [Environment]::SetEnvironmentVariable('FOLDERSKIN_INSTALL_REFRESH', '1', 'User')
    [Environment]::SetEnvironmentVariable('FOLDERSKIN_INSTALL_REFRESH', $null, 'User')
    Say "added $InstallDir to your user PATH; new terminals will find folderskin"
}
if (($env:Path -split ';') -notcontains $InstallDir) {
    $env:Path = "$env:Path;$InstallDir"
}

Say "installed folderskin $v in $InstallDir"
& (Join-Path $InstallDir 'folderskin.exe') --version
Say 'next: folderskin ai doctor'
