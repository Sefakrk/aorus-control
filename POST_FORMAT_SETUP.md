# Windows 11 Post-Format Automation Guide & Agent Playbook

> **Target Machine:** Gigabyte AORUS 16X 2024 (13th Gen Intel HX, RTX 4060, 240Hz/165Hz)  
> **Purpose:** Single-prompt automated restoration of Windows 11 settings, debloat, package installation via `winget`, and deployment of `aorus-control`.  
> **Author:** Antigravity / Sefakrk  

---

## 1. Top Open-Source Replacements for Windows 11 Bloat

Before building something from scratch, these existing battle-tested, open-source projects replace bloated Windows 11 components with **zero RAM overhead**:

| Windows 11 Bloatware | Open-Source Alternative | Neden Çok Daha İyi? | Winget ID / Kaynak |
| :--- | :--- | :--- | :--- |
| **Windows Ses Mikseri & EarTrumpet** | **Volume2** | C# değil, saf C++ ile yazılmıştır. Sadece 3 MB RAM tüketir. Saatin yanında anlık uygulama bazlı ses kaydırıcıları ve klavye kısayoluyla kulaklık/hoparlör geçişi sağlar. | `irzyxa.Volume2` |
| **Windows Arama / Dosya Arama** | **Everything (Voidtools)** | Windows İndeksleme Servisi disk ve CPU sömürürken, Everything NTFS MFT'yi doğrudan okuyarak 1 milyon dosyayı 0 milisaniyede arar. | `voidtools.Everything` |
| **PowerToys / Raycast** | **Flow Launcher** | Alt+Space ile açılan, Python ve C# eklentilerini destekleyen, 20 MB RAM tüketen süper hızlı uygulama ve dosya başlatıcı. | `Flow-Launcher.Flow-Launcher` |
| **Windows Ekran Alıntısı** | **Flameshot** | Windows Snipping Tool takılırken Flameshot anında ekranı dondurur; ok, kutu, metin ve otomatik bulanıklaştırma (blur) araçlarıyla panoya kopyalar. | `Flameshot.Flameshot` |
| **Dosya Gezgini ZIP / WinRAR** | **NanaZip** | Windows 11'in yeni sağ tık menüsüne yerel entegre olan, 7-Zip çekirdekli modern ve kayıpsız arşiv yöneticisi. | `M2Team.NanaZip` |
| **GCC & RGB Fusion** | **aorus-control** | Bizim geliştirdiğimiz 15 MB RAM'li yerel Rust aracı; klavye/arka bar RGB, fan eğrisi, MUX, ekran Hz ve %80 pil korumasını sıfır servisle yönetir. | [GitHub Repo](https://github.com/Sefakrk/aorus-control) |

---

## 2. Automated Post-Format PowerShell Script (`post_format_setup.ps1`)

Aşağıdaki script format sonrası **Terminal (Yönetici)** açılarak doğrudan çalıştırılabilir:

```powershell
<#
.SYNOPSIS
    AORUS 16X Windows 11 Post-Format Master Setup Script
.DESCRIPTION
    Applies privacy debloat, explorer tweaks, installs user app stack via winget,
    and deploys aorus-control with Secure Boot WMI registration.
#>

# 1. Elevate to Administrator
if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Warning "Bu script Yonetici olarak calistirilmalidir! Yeniden baslatiliyor..."
    Start-Process powershell -Verb RunAs -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`""
    exit
}

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "   AORUS 16X - Windows 11 Post-Format Setup Baslatiliyor  " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

# -------------------------------------------------------------------------
# ASAMA 1: Windows 11 Ayarlari, Gizlilik & Debloat
# -------------------------------------------------------------------------
Write-Host "`n[1/4] Windows 11 Kayit Defteri & Explorer Ayarlari Yapiliyor..." -ForegroundColor Yellow

# A. Sistem Genelinde Karanlik Tema (Dark Mode)
Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize" -Name "AppsUseLightTheme" -Value 0 -Type DWord
Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize" -Name "SystemUsesLightTheme" -Value 0 -Type DWord

# B. Klasik Sag Tik Menusu (Windows 10 Tarzi Tam Menu)
$regPath = "HKCU:\Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32"
New-Item -Path $regPath -Force | Out-Null
Set-ItemProperty -Path $regPath -Name "(Default)" -Value ""

# C. Dosya Gezgini Ayarlari (Uzantilari Goster, Gizli Dosyalari Goster)
Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced" -Name "HideFileExt" -Value 0 -Type DWord
Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced" -Name "Hidden" -Value 1 -Type DWord
Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced" -Name "LaunchTo" -Value 1 -Type DWord # 'Bu Bilgisayar' olarak ac

# D. Gorev Cubugu Temizligi (Copilot, Widget, Task View Gizle)
Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced" -Name "ShowTaskViewButton" -Value 0 -Type DWord
Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced" -Name "TaskbarDa" -Value 0 -Type DWord # Widget/Haberler
Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced" -Name "ShowCopilotButton" -Value 0 -Type DWord -ErrorAction SilentlyContinue

# E. Telemetri ve Geri Bildirim Kisitlama
Set-ItemProperty -Path "HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection" -Name "AllowTelemetry" -Value 0 -Type DWord -ErrorAction SilentlyContinue

# F. Nihai Performans (Ultimate Performance) Guc Planini Ac
powercfg -duplicatescheme e9a42b02-d5df-448d-aa00-03f14749eb61 | Out-Null

Write-Host "-> Windows arayuz ve performans ayarlari tamamlandi." -ForegroundColor Green

# -------------------------------------------------------------------------
# ASAMA 2: Temel Gelistirici, Guvenlik ve Sistem Paketleri (Winget)
# -------------------------------------------------------------------------
Write-Host "`n[2/4] Uygulamalar Winget Uzerinden Kuruluyor..." -ForegroundColor Yellow

$packages = @(
    # Gelistirici Araclari
    "Git.Git",
    "GitHub.cli",
    "Rustlang.Rustup",
    "OpenJS.NodeJS.LTS",
    "Python.Python.3.14",
    "BurntSushi.ripgrep.MSVC",
    "BrechtSanders.WinLibs.POSIX.UCRT",
    "Docker.DockerDesktop",
    "Microsoft.WSL",

    # Kod Editorleri & AI
    "Anysphere.Cursor",
    "ZedIndustries.Zed",
    "VSCodium.VSCodium",
    "Anthropic.Claude",

    # Tarayicilar
    "Zen-Team.Zen-Browser",
    "ImputNet.Helium",

    # Guvenlik, Not & Verimlilik
    "Bitwarden.Bitwarden",
    "Obsidian.Obsidian",
    "Google.GoogleDrive",

    # Medya & Sistem Yardimcilari
    "7zip.7zip",
    "AntibodySoftware.WizTree",
    "qBittorrent.qBittorrent",
    "VideoLAN.VLC",
    "HandBrake.HandBrake",
    "CharlesMilette.TranslucentTB",
    "voidtools.Everything",
    "irzyxa.Volume2",

    # Iletisim & Oyun
    "SpikeHD.Dorion",
    "Telegram.TelegramDesktop",
    "Spotify.Spotify",
    "Valve.Steam",
    "HeroicGamesLauncher.HeroicGamesLauncher",
    "ElectronicArts.EADesktop"
)

foreach ($pkg in $packages) {
    Write-Host "-> Kuruluyor: $pkg ..." -ForegroundColor Cyan
    winget install --id $pkg -e --silent --accept-package-agreements --accept-source-agreements --force
}

Write-Host "-> Tum yazilim paketleri kuruldu." -ForegroundColor Green

# -------------------------------------------------------------------------
# ASAMA 3: aorus-control (GCC Yerine Donanim & Fan & RGB Araci) Kurulumu
# -------------------------------------------------------------------------
Write-Host "`n[3/4] Aorus Control Derleniyor ve Sisteme Entegre Ediliyor..." -ForegroundColor Yellow

$repoDir = "C:\Users\$env:USERNAME\Desktop\Repolar\aorus-control"
if (-not (Test-Path $repoDir)) {
    New-Item -ItemType Directory -Path (Split-Path $repoDir) -Force | Out-Null
    git clone https://github.com/Sefakrk/aorus-control.git $repoDir
}

# Rust araci ile release derleme
Push-Location $repoDir
$binDir = (Get-ChildItem -Path "$env:LOCALAPPDATA\Microsoft\WinGet\Packages" -Recurse -Filter "gcc.exe" | Select-Object -First 1).DirectoryName
if ($binDir) { $env:Path = "$binDir;$env:Path" }

cargo build --release

# WMI Binary MOF Kaydi (Secure Boot destegi icin)
if (Test-Path "assets\acpi.bmof") {
    mofcomp.exe -N:root\wmi "assets\acpi.bmof" | Out-Null
}

Pop-Location

# Masaustu Yonetici Kisayolu Olustur
$exePath = "$repoDir\target\release\aorus-control.exe"
$lnkPath = "C:\Users\$env:USERNAME\Desktop\Aorus Control.lnk"

$wsh = New-Object -ComObject WScript.Shell
$shortcut = $wsh.CreateShortcut($lnkPath)
$shortcut.TargetPath = $exePath
$shortcut.WorkingDirectory = $repoDir
$shortcut.IconLocation = "$repoDir\assets\icon.ico, 0"
$shortcut.Save()

# Yonetici Olarak Calistir Bayragini Isle
$bytes = [System.IO.File]::ReadAllBytes($lnkPath)
$bytes[0x15] = $bytes[0x15] -bor 0x20
[System.IO.File]::WriteAllBytes($lnkPath, $bytes)

# Gorev Zamanlayiciya Ekle (Windows Acildiginda Sessizce Tray'de Baslasin)
$action = New-ScheduledTaskAction -Execute $exePath
$trigger = New-ScheduledTaskTrigger -AtLogOn
$principal = New-ScheduledTaskPrincipal -UserId "$env:USERDOMAIN\$env:USERNAME" -RunLevel Highest
Register-ScheduledTask -TaskName "AorusControlTray" -Action $action -Trigger $trigger -Principal $principal -Force | Out-Null

Write-Host "-> aorus-control basariyla derlendi, kisayol ve otomatik baslatma olusturuldu." -ForegroundColor Green

# -------------------------------------------------------------------------
# ASAMA 4: Explorer'i Yeniden Baslat
# -------------------------------------------------------------------------
Write-Host "`n[4/4] Degisikliklerin Etkili Olmasi Icin Windows Gezgini Yenileniyor..." -ForegroundColor Yellow
Stop-Process -Name explorer -Force

Write-Host "`n==========================================================" -ForegroundColor Green
Write-Host "   KURULUM TAMAMLANDI! Windows tertemiz ve hazir.        " -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Green
```

---

## 3. Format Sonrası Yapay Zeka Ajanına Verilecek Prompt (Kopyala & Yapıştır)

Herhangi bir AI ajanı (omp, Claude Code, Cursor vb.) açtığında şu tek paragrafı göndermen yeterlidir:

```text
Sistem yeni formatlandı (Windows 11 temiz kurulum). Senden sistemi benim kişisel standartlarıma göre otomatik yapılandırmanı istiyorum:
1. Masaüstündeki veya repodaki 'POST_FORMAT_SETUP.md' dökümanındaki adımları izle.
2. Windows ayarlarını uygula: Sistem genelinde Dark Mode, klasik sağ tık bağlam menüsü, dosya gezgininde uzantıları ve gizli dosyaları gösterme, Copilot/Widget görev çubuğu simgelerini kapatma.
3. Dökümanda listelenen temel paketleri (Git, Rustup, Cursor, Zed, Zen Browser, Helium, Bitwarden, Obsidian, Docker, VLC, Volume2 vb.) winget üzerinden sessiz modda kur.
4. 'https://github.com/Sefakrk/aorus-control.git' reposunu klonla, 'cargo build --release' ile derle, 'assets/acpi.bmof' şemasını mofcomp ile kaydet, masaüstü kısayolunu ve görev zamanlayıcı başlangıç görevini ayarla.
5. İşlem bitince Explorer'ı yeniden başlatıp test et.
```
