#Requires -Version 7.0
param([switch]$Signed)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
function Run([string]$Exe, [string[]]$Arguments) {
    & $Exe @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Exe failed with exit code $LASTEXITCODE" }
}
$Root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
Set-Location $Root
$Version = (Select-String -Path packaging/release/sources.env -Pattern '^APP_VERSION=(.*)$').Matches.Groups[1].Value
$FfmpegVersion = (Select-String -Path packaging/release/sources.env -Pattern '^FFMPEG_VERSION=(.*)$').Matches.Groups[1].Value
$Dav1dVersion = (Select-String -Path packaging/release/sources.env -Pattern '^DAV1D_VERSION=(.*)$').Matches.Groups[1].Value
$ZlibVersion = (Select-String -Path packaging/release/sources.env -Pattern '^ZLIB_VERSION=(.*)$').Matches.Groups[1].Value
$Target = 'x86_64-pc-windows-msvc' 
$Vendor = Join-Path $Root "packaging/vendor/$Target"
foreach ($tool in 'cargo','rustc','dumpbin','rc','ISCC','tar') { Get-Command $tool -ErrorAction Stop | Out-Null }
if ($Signed) {
    if ($env:UFFMPEG_CERT_THUMBPRINT -notmatch '^[A-Fa-f0-9]{40}$') { throw 'Set UFFMPEG_CERT_THUMBPRINT to a code-signing certificate thumbprint.' }
    if ($env:UFFMPEG_TIMESTAMP_URL -notmatch '^https://[A-Za-z0-9./:_-]+$') { throw 'Set UFFMPEG_TIMESTAMP_URL to your trusted HTTPS timestamp service.' }
    Get-Command signtool -ErrorAction Stop | Out-Null
}
# Helpers must have been created and audited by build-helpers.sh in MSYS2 UCRT64.
foreach ($relative in 'bin/ffmpeg.exe','bin/ffprobe.exe','build-info.txt','corresponding-source.txt','SHA256SUMS','licenses/FFmpeg-LGPL-2.1.txt', 'licenses/dav1d-COPYING',"source/ffmpeg-$FfmpegVersion.tar.xz","source/dav1d-$Dav1dVersion.tar.xz","source/zlib-$ZlibVersion.tar.xz","licenses/zlib-LICENSE.h") {
    $file = Get-Item (Join-Path $Vendor $relative)
    if ($file.LinkType -or $file.Length -eq 0) { throw "Invalid release input: $relative" }
}
foreach ($line in Get-Content (Join-Path $Vendor 'SHA256SUMS')) {
    if ($line -notmatch '^([0-9a-f]{64})  ((bin|source)/[^\r\n]+)$') { throw 'Malformed helper checksum manifest' }
    $expected = $Matches[1]; $relative = $Matches[2]
    if ($relative.Contains('..')) { throw 'Invalid checksum path' }
    if ((Get-FileHash (Join-Path $Vendor $relative) -Algorithm SHA256).Hash -ne $expected) { throw "Checksum mismatch: $relative" }
}
foreach ($helper in 'ffmpeg','ffprobe') {
    $details = & "$Vendor/bin/$helper.exe" -version
    if ($LASTEXITCODE -ne 0 -or $details[0] -notmatch ('^' + [regex]::Escape($helper) + ' version n?' + [regex]::Escape($FfmpegVersion) + '(?:\s|$)')) { throw "Wrong $helper version" }
    $configuration = $details -join "`n"
    if ($configuration -match '--enable-(gpl|nonfree|version3)' -or $configuration -notmatch '--disable-autodetect' -or $configuration -notmatch '--enable-libdav1d') { throw 'Unapproved helper configuration' }
    if (-not (Select-String -Path "$Vendor/SHA256SUMS" -SimpleMatch "  bin/$helper.exe")) { throw 'Missing helper checksum' }
}
$Tag = if ($Signed) { 'signed' } else { 'unsigned' }
$Output = Join-Path $Root "dist/windows-$Tag"
if (Test-Path $Output) { throw "Output already exists: $Output. Move it aside first." }
$Stage = Join-Path $Root ('work/release/windows-' + [guid]::NewGuid().ToString('N'))
$App = Join-Path $Stage 'app'
$Records = Join-Path $Stage 'records'
New-Item -ItemType Directory -Path $App,$Output -Force | Out-Null
$env:UFFMPEG_LICENSE_INPUTS = "$Records.license-inputs"
Run 'python' @('scripts/collect-upstream-licenses.py',$Target,$env:UFFMPEG_LICENSE_INPUTS)
Run 'cargo' @('run','--release','--locked','-p','frameforge-application','--bin','release-records','--',$Records,$Target)
$ProjectSource = Join-Path $Records 'source/project'
New-Item -ItemType Directory -Path "$ProjectSource/.cargo" -Force | Out-Null
$config = & cargo vendor --locked --versioned-dirs "$ProjectSource/rust-vendor"
if ($LASTEXITCODE -ne 0) { throw 'cargo vendor failed' }
($config -replace 'directory = ".*"','directory = "rust-vendor"') | Set-Content "$ProjectSource/.cargo/config.toml" -Encoding utf8
Copy-Item Cargo.toml,Cargo.lock,LICENSE,README.md -Destination $ProjectSource
foreach ($directory in 'apps','crates','assets','docs','sprint','scripts') { Copy-Item $directory $ProjectSource -Recurse }
New-Item -ItemType Directory -Path "$ProjectSource/packaging" -Force | Out-Null
Copy-Item packaging/release,packaging/icons,packaging/windows.rc,packaging/Info.plist,packaging/uffmpeg.desktop -Destination "$ProjectSource/packaging" -Recurse
$env:UFFMPEG_EMBED_WINDOWS_ICON = '1'
$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS = '-C target-feature=+crt-static' 
Run 'cargo' @('build','--release','--locked','--target',$Target,'-p','frameforge-desktop')
Copy-Item "target/$Target/release/frameforge-desktop.exe" "$App/uffmpeg.exe"
Copy-Item $Vendor "$App/ffmpeg" -Recurse
Copy-Item "$Records/licenses" $App -Recurse
Copy-Item "$Records/THIRD-PARTY.txt","$Records/dependencies.json",LICENSE -Destination $App
Copy-Item packaging/icons/uffmpeg.ico $App
'Exact Rust source and vendored dependencies are in the companion source archive under records/source/project. FFmpeg and dav1d source archives are installed in ffmpeg/source.' | Set-Content "$App/SOURCE-LOCATION.txt" -Encoding utf8
Run 'rustc' @('-vV')
# Reject non-system DLLs; do not silently rely on a developer's PATH or VC redistributable.
$system = @('kernel32.dll','user32.dll','advapi32.dll','shell32.dll','ole32.dll','oleaut32.dll','ws2_32.dll','bcrypt.dll','bcryptprimitives.dll','secur32.dll','ntdll.dll','ucrtbase.dll','msvcrt.dll','gdi32.dll','comdlg32.dll','comctl32.dll','dwmapi.dll','imm32.dll','shlwapi.dll','version.dll','winmm.dll','opengl32.dll','d3d11.dll','dxgi.dll','dwrite.dll','d2d1.dll','d3dcompiler_47.dll','propsys.dll','uxtheme.dll','setupapi.dll','cfgmgr32.dll','powrprof.dll','crypt32.dll','normaliz.dll','msimg32.dll','winspool.drv','runtimeobject.dll','windowscodecs.dll')
foreach ($exe in Get-ChildItem $App -Filter *.exe -Recurse) {
    $imports = & dumpbin /DEPENDENTS $exe.FullName
    if ($LASTEXITCODE -ne 0) { throw 'Dependency audit failed' }
    foreach ($line in $imports) {
        if ($line.Trim() -match '^[A-Za-z0-9_.-]+\.(dll|drv)$') {
            $dll = $line.Trim().ToLowerInvariant()
            if ($dll -notin $system -and $dll -notlike 'api-ms-win-*.dll' -and $dll -notlike 'ext-ms-win-*.dll') { throw "Unbundled dependency: $dll in $($exe.Name). Use static CRT or explicitly package and license that dependency." }
        }
    }
    if ($Signed) { Run 'signtool' @('sign','/sha1',$env:UFFMPEG_CERT_THUMBPRINT,'/fd','SHA256','/tr',$env:UFFMPEG_TIMESTAMP_URL,'/td','SHA256',$exe.FullName) }
}
if ($Signed) { Rename-Item "$App/ffmpeg/SHA256SUMS" 'UNSIGNED-BUILD-SHA256SUMS' }
Run "$App/uffmpeg.exe" @('--diagnostics')
$Arguments = @("/DStage=$App","/DOutput=$Output","/DAppVersion=$Version")
if ($Signed) {
    $Arguments += '/DSigned=1'
    $Arguments += ('/Srelease=signtool.exe sign /sha1 ' + $env:UFFMPEG_CERT_THUMBPRINT + ' /fd SHA256 /tr ' + $env:UFFMPEG_TIMESTAMP_URL + ' /td SHA256 $f')
}
$Arguments += (Join-Path $PSScriptRoot 'windows.iss')
Run 'ISCC' $Arguments
$Installer = Join-Path $Output "uFFmpeg-$Version-windows-x64.exe"
if ($Signed) { Run 'signtool' @('verify','/pa','/all',$Installer) }
Run 'tar' @('-czf',"$Output/uFFmpeg-$Version-source.tar.gz",'-C',$Stage,'records')
$PublishedFiles = @(Get-ChildItem $Output -File)
$PublishedFiles | ForEach-Object { "$((Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant())  $($_.Name)" } | Set-Content "$Output/SHA256SUMS" -Encoding utf8
Write-Output "Created $Installer. Clean-machine installation review is still required."
