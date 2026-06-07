$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
$VcVars = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
$CargoBin = Join-Path $env:USERPROFILE ".cargo\bin"

if (-not (Test-Path $VcVars)) {
    Write-Error "MSVC Build Tools not found. Install Visual Studio 2022 Build Tools with C++ workload."
}

$env:Path = "$CargoBin;$env:Path"

Write-Host "Building frontend..." -ForegroundColor Cyan
Set-Location $Root
npm run build

Write-Host "Building Tauri release..." -ForegroundColor Cyan
npm run tauri build

Write-Host "Building silent launcher..." -ForegroundColor Cyan
cmd /c "`"$VcVars`" && set PATH=$CargoBin;%PATH% && cd /d `"$Root\launcher`" && cargo build --release"

$OutDir = Join-Path $Root "GamingNewsPublisher"
$AppDir = Join-Path $OutDir "app"

if (Test-Path $OutDir) {
    Get-ChildItem $OutDir -Exclude "app" | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
    if (Test-Path $AppDir) {
        Get-ChildItem $AppDir -Exclude "data","llm" | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
    }
}
New-Item -ItemType Directory -Path $AppDir -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $AppDir "llm\bin") -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $AppDir "llm/models") -Force | Out-Null

$BundledServer = Join-Path $Root "resources/llm/llama-server.exe"
if (Test-Path $BundledServer) {
    Copy-Item $BundledServer (Join-Path $AppDir "llm/bin/llama-server.exe")
}

$ReleaseExe = Join-Path $Root "src-tauri\target\release\gaming-news-publisher.exe"
if (-not (Test-Path $ReleaseExe)) {
    Write-Error "Release binary not found: $ReleaseExe"
}

Copy-Item $ReleaseExe (Join-Path $AppDir "Gaming News Publisher.exe")
Copy-Item (Join-Path $Root "src-tauri\icons\icon.ico") $OutDir

$LauncherSrc = Join-Path $Root "launcher\target\release\gaming-news-launcher.exe"
if (-not (Test-Path $LauncherSrc)) {
    Write-Error "Launcher binary not found: $LauncherSrc"
}
Copy-Item $LauncherSrc (Join-Path $OutDir "Gaming News Publisher.exe")

$NsisDir = Join-Path $Root "src-tauri\target\release\bundle\nsis"
if (Test-Path $NsisDir) {
    Get-ChildItem $NsisDir -Filter "*.exe" | ForEach-Object {
        Copy-Item $_.FullName $OutDir
    }
}

@"
Gaming News Publisher - Portable
================================

Запуск:
  Дважды кликните "Gaming News Publisher.exe" в этой папке.
  Launcher запускает программу без консольных окон.

Содержимое:
  Gaming News Publisher.exe  - тихий launcher (без cmd-окна)
  app\Gaming News Publisher.exe - основное приложение
  *-setup.exe (если есть) - установщик с WebView2

Данные приложения хранятся в:
  app\data\
  (gaming_news.db, settings.json)
  app\llm\
  (llama-server.exe, models — локальный AI)

При ошибке запуска смотрите launcher-error.log в этой папке.
"@ | Set-Content (Join-Path $OutDir "README.txt") -Encoding UTF8

Write-Host ""
Write-Host "Portable package ready:" -ForegroundColor Green
Write-Host $OutDir
Write-Host ""
Write-Host "Run: $(Join-Path $OutDir 'Gaming News Publisher.exe')" -ForegroundColor Yellow
