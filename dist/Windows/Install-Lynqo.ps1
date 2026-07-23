# Lynqo Windows Production Installation Script
$ErrorActionPreference = "Stop"

$installDir = "$env:LOCALAPPDATA\Lynqo"
Write-Host "Installing Lynqo Central Storage Hub to $installDir..." -ForegroundColor Cyan

New-Item -ItemType Directory -Force -Path $installDir | Out-Null
Copy-Item -Path "$PSScriptRoot\*" -Destination $installDir -Recurse -Force

# Create Desktop Shortcut pointing to silent VBScript launcher (No Terminal Window)
$wsh = New-Object -ComObject WScript.Shell
$shortcut = $wsh.CreateShortcut("$env:USERPROFILE\Desktop\Lynqo.lnk")
$shortcut.TargetPath = "wscript.exe"
$shortcut.Arguments = """$installDir\Lynqo-Windows.vbs"""
$shortcut.WorkingDirectory = $installDir
$shortcut.Description = "Lynqo Central Storage Hub"
$shortcut.Save()

Write-Host "Lynqo successfully installed!" -ForegroundColor Green
Write-Host "Double-click 'Lynqo' on your Desktop to run." -ForegroundColor Yellow
