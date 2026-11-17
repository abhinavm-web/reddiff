$ErrorActionPreference = "Stop"

$Repo = "abhinavm-web/reddiff"
$BinName = "reddiff"
$Target = "x86_64-pc-windows-msvc"
$Archive = "$BinName-$Target.zip"
$Url = "https://github.com/$Repo/releases/latest/download/$Archive"

$InstallDir = if ($env:INSTALL_DIR) { $env:INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "Programs\$BinName\bin" }
$TmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("install_" + $BinName + "_" + [guid]::NewGuid().ToString("N").Substring(0, 8))

try {
    New-Item -ItemType Directory -Path $TmpDir -Force | Out-Null
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null

    $ArchivePath = Join-Path $TmpDir $Archive
    Write-Host "Downloading $BinName ($Target) from $Url..."
    Invoke-WebRequest -Uri $Url -OutFile $ArchivePath -UseBasicParsing

    $ExtractDir = Join-Path $TmpDir "extracted"
    Expand-Archive -LiteralPath $ArchivePath -DestinationPath $ExtractDir -Force

    Copy-Item -LiteralPath (Join-Path $ExtractDir "$BinName.exe") -Destination (Join-Path $InstallDir "$BinName.exe") -Force
    Write-Host "Installed $BinName.exe to $InstallDir"

    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $PathParts = if ($UserPath) { $UserPath -split ";" | Where-Object { $_ -ne "" } } else { @() }
    if ($PathParts -notcontains $InstallDir) {
        $NewUserPath = ($PathParts + $InstallDir) -join ";"
        [Environment]::SetEnvironmentVariable("Path", $NewUserPath, "User")
        Write-Host "Added $InstallDir to user PATH."
    }
    if (($env:Path -split ";") -notcontains $InstallDir) {
        $env:Path = "$InstallDir;$env:Path"
    }
}
finally {
    if (Test-Path -LiteralPath $TmpDir) {
        Remove-Item -LiteralPath $TmpDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
