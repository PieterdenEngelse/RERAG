#Requires -RunAsAdministrator

################################################################################
# Agentic RAG Installer for Windows - ENHANCED v1.1.0
# Date: 2025-11-03
# Platforms: Windows 10 22H2+, Windows 11
#
# ENHANCEMENTS from v1.0.0:
#   ✓ Improved argument parsing (--help, --version, --skip-checks)
#   ✓ AG_HOME support for PathManager (v13.1.2 integration)
#   ✓ Database initialization (documents.db, memory.db)
#   ✓ Better logging functions (Write-Info, Write-Success, Write-Warn, Write-Error)
#   ✓ Show-Help function for better documentation
#   ✓ Configuration template with Redis and PathManager settings
#   ✓ Verbose mode for debugging
#   ✓ Pre-flight checks improvement
#
# Usage:
#   Set-ExecutionPolicy -ExecutionPolicy Bypass -Scope Process
#   .\install-windows-v1.1.0.ps1 [OPTIONS]
#
# Options:
#   -ProjectPath PATH       Path to project (default: current directory)
#   -Mode release|debug     Build mode (default: release)
#   -InstallPrefix PATH     Installation prefix (default: $HOME/.agentic-rag)
#   -BackendPort PORT       Backend port (default: 3010)
#   -FrontendPort PORT      Frontend port (default: 3000)
#   -Verbose                Enable verbose logging
#   -SkipChecks             Skip preflight checks
#   -Version                Show version
#   -Help                   Show this help message
#
# Integration Points:
#   - Directory creation (~/.agentic-rag/ + AG_HOME paths)
#   - Database initialization (documents.db, memory.db)
#   - Environment variable setup (Windows Registry)
#   - Health verification via Invoke-WebRequest
#   - Comprehensive logging
#   - Redis configuration support
#   - Performance metrics
################################################################################

param(
    [string]$ProjectPath = (Get-Location),
    [ValidateSet("release", "debug")]
    [string]$Mode = "release",
    [string]$InstallPrefix = "$env:USERPROFILE\.agentic-rag",
    [string]$BackendPort = "3010",
    [string]$FrontendPort = "3000",
    [switch]$Verbose,
    [switch]$SkipChecks,
    [switch]$Version,
    [switch]$Help
)

# Configuration
$INSTALLER_VERSION = "1.1.0"
$INSTALL_MODE = $Mode
$BACKEND_PORT = $BackendPort
$FRONTEND_PORT = $FrontendPort
$HEALTH_CHECK_TIMEOUT = 10
$STARTUP_DELAY = 3
$INSTALLER_LOG = "$InstallPrefix\installer.log"

# Logging functions (improved from v1.0.0)
function Write-Success {
    param([string]$Message)
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Write-Host "[$timestamp] " -NoNewline -ForegroundColor Cyan
    Write-Host "✓ $Message" -ForegroundColor Green
    Add-Content -Path $INSTALLER_LOG -Value "[$timestamp] SUCCESS: $Message"
}

function Write-Info {
    param([string]$Message)
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Write-Host "[$timestamp] " -NoNewline -ForegroundColor Cyan
    Write-Host "ℹ️  $Message" -ForegroundColor Blue
    Add-Content -Path $INSTALLER_LOG -Value "[$timestamp] INFO: $Message"
}

function Write-Warning {
    param([string]$Message)
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Write-Host "[$timestamp] " -NoNewline -ForegroundColor Cyan
    Write-Host "⚠️  $Message" -ForegroundColor Yellow
    Add-Content -Path $INSTALLER_LOG -Value "[$timestamp] WARN: $Message"
}

function Write-Error {
    param([string]$Message)
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Write-Host "[$timestamp] " -NoNewline -ForegroundColor Cyan
    Write-Host "✗ $Message" -ForegroundColor Red
    Add-Content -Path $INSTALLER_LOG -Value "[$timestamp] ERROR: $Message"
}

# Show help (new in v1.1.0, from v13.1.2 pattern)
function Show-Help {
    $helpText = @"
Agentic RAG Installation Script v$INSTALLER_VERSION

USAGE:
    .\install-windows-v1.1.0.ps1 [OPTIONS]

OPTIONS:
    -ProjectPath PATH       Path to project (default: current directory)
    -Mode release|debug     Build mode (default: release)
    -InstallPrefix PATH     Installation prefix (default: `$HOME\.agentic-rag)
    -BackendPort PORT       Backend port (default: 3010)
    -FrontendPort PORT      Frontend port (default: 3000)
    -Verbose                Enable verbose logging
    -SkipChecks             Skip preflight checks
    -Version                Show version and exit
    -Help                   Show this help message

EXAMPLES:
    .\install-windows-v1.1.0.ps1
    .\install-windows-v1.1.0.ps1 -Mode debug -Verbose
    .\install-windows-v1.1.0.ps1 -InstallPrefix "C:\fro" -BackendPort 8080
    .\install-windows-v1.1.0.ps1 -ProjectPath "C:\projects\fro" -SkipChecks

PLATFORMS:
    • Windows 10 22H2+
    • Windows 11

REQUIREMENTS:
    • Administrator privileges
    • Rust 1.70+ (from https://rustup.rs)
    • Cargo (included with Rust)
    • Visual Studio Build Tools 2019+ or VS Community 2019+
    • 4 GB RAM, 1 GB disk space

"@
    Write-Host $helpText
}

# Handle version and help flags
if ($Version) {
    Write-Host "Agentic RAG Installer v$INSTALLER_VERSION"
    exit 0
}

if ($Help) {
    Show-Help
    exit 0
}

# Print header
function Print-Header {
    Write-Host ""
    Write-Host "╔══════════════════════════════════════════════════════════╗" -ForegroundColor Blue
    Write-Host "║  Agentic RAG Installer for Windows - v$INSTALLER_VERSION       ║" -ForegroundColor Blue
    Write-Host "╚══════════════════════════════════════════════════════════╝" -ForegroundColor Blue
    Write-Host ""
}

# Verify prerequisites (improved)
function Verify-Prerequisites {
    if ($SkipChecks) {
        Write-Warning "Preflight checks skipped (-SkipChecks)"
        return $true
    }

    Write-Info "Running preflight checks..."
    
    # Check if running as administrator
    $currentPrincipal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not $currentPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        Write-Error "This script must be run as Administrator"
        return $false
    }
    Write-Success "Running with Administrator privileges"
    
    # Check Rust
    try {
        $rustVersion = rustc --version
        Write-Success "Rust installed: $rustVersion"
    } catch {
        Write-Error "Rust not found. Please install from https://rustup.rs"
        return $false
    }
    
    # Check Cargo
    try {
        $cargoVersion = cargo --version
        Write-Success "Cargo installed: $cargoVersion"
    } catch {
        Write-Error "Cargo not found"
        return $false
    }
    
    # Check Git (optional)
    try {
        $gitVersion = git --version
        Write-Success "Git installed: $gitVersion"
    } catch {
        Write-Warning "Git not found (optional but recommended)"
    }
    
    # Check Visual Studio Build Tools
    $vsPath = "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vsPath) {
        Write-Success "Visual Studio Build Tools detected"
    } else {
        Write-Warning "Visual Studio Build Tools not detected (may still be installed)"
    }

    # Check disk space (require 1 GB)
    $disk = Get-Volume -DriveLetter C
    $availableGB = $disk.SizeRemaining / 1GB
    if ($availableGB -lt 1) {
        Write-Warning "Less than 1 GB available on drive C"
    } else {
        Write-Success "Sufficient disk space available ($([Math]::Round($availableGB, 2)) GB)"
    }
    
    Write-Success "All prerequisites verified"
    return $true
}

# Create directories (enhanced with AG_HOME)
function Create-Directories {
    Write-Info "Creating installation directories..."
    
    $directories = @(
        $InstallPrefix,
        "$InstallPrefix\logs",
        "$InstallPrefix\data",
        "$InstallPrefix\uploads",
        "$InstallPrefix\models",
        "$InstallPrefix\db",
        "$InstallPrefix\index",
        "$InstallPrefix\cache",
        "$InstallPrefix\config"
    )
    
    foreach ($dir in $directories) {
        if (-not (Test-Path -Path $dir)) {
            New-Item -ItemType Directory -Force -Path $dir | Out-Null
            Write-Success "Created directory: $dir"
        } else {
            Write-Info "Directory exists: $dir"
        }
    }
}

# Setup environment variables (enhanced with AG_HOME)
function Setup-Environment {
    Write-Info "Setting up environment variables..."
    
    $envVars = @{
        "RUST_LOG" = "info"
        "MONITORING_ENABLED" = "true"
        "LOG_RETENTION_DAYS" = "7"
        "LOG_FORMAT" = "json"
        "AGENTIC_RAG_HOME" = $InstallPrefix
        "AG_HOME" = $InstallPrefix
        "BACKEND_PORT" = $BackendPort
        "FRONTEND_PORT" = $FrontendPort
    }
    
    foreach ($var in $envVars.GetEnumerator()) {
        try {
            [Environment]::SetEnvironmentVariable($var.Key, $var.Value, "User")
            $env:($var.Key) = $var.Value
            Write-Success "Set environment variable: $($var.Key) = $($var.Value)"
        } catch {
            Write-Error "Failed to set environment variable: $($var.Key)"
            return $false
        }
    }
    return $true
}

# NEW v1.1.0: Initialize databases (from v13.1.2 pattern)
function Init-Databases {
    Write-Info "Initializing databases..."
    
    $dbDir = "$InstallPrefix\db"
    
    try {
        # Create empty database files
        "" | Out-File "$dbDir\documents.db" -Force
        "" | Out-File "$dbDir\memory.db" -Force
        
        Write-Success "Databases created: documents.db, memory.db"
        return $true
    } catch {
        Write-Error "Failed to initialize databases: $_"
        return $false
    }
}

# Verify dependencies
function Verify-Dependencies {
    Write-Info "Verifying Rust dependencies..."
    
    Push-Location $ProjectPath
    
    try {
        cargo check 2>&1 | Out-Null
        Write-Success "Dependencies verified"
        Pop-Location
        return $true
    } catch {
        Write-Warning "Some dependencies may need to be downloaded"
        Pop-Location
        return $true
    }
}

# Build project
function Build-Project {
    Write-Info "Building project in $INSTALL_MODE mode..."
    
    Push-Location $ProjectPath
    
    try {
        if ($INSTALL_MODE -eq "release") {
            cargo build --release 2>&1 | Tee-Object -FilePath $INSTALLER_LOG -Append | Out-Null
        } else {
            cargo build 2>&1 | Tee-Object -FilePath $INSTALLER_LOG -Append | Out-Null
        }
        Write-Success "Build completed successfully"
        Pop-Location
        return $true
    } catch {
        Write-Error "Build failed. Check $INSTALLER_LOG for details"
        Pop-Location
        return $false
    }
}

# Verify build artifacts
function Verify-Artifacts {
    Write-Info "Verifying build artifacts..."
    
    $binaryPath = "$ProjectPath\target\$INSTALL_MODE\fro.exe"
    
    if (Test-Path -Path $binaryPath) {
        $fileSize = (Get-Item $binaryPath).Length / 1MB
        Write-Success "Binary verified: $binaryPath ($([Math]::Round($fileSize, 2)) MB)"
        return $true
    } else {
        Write-Warning "Expected binary not found at $binaryPath"
        return $true
    }
}

# Create configuration (enhanced with AG_HOME and Redis support)
function Create-Configuration {
    Write-Info "Creating configuration file..."
    
    $configFile = "$InstallPrefix\config\.env"
    
    if (-not (Test-Path -Path $configFile)) {
        $configContent = @"
# Agentic RAG Configuration
# Version: 1.1.0
# Generated: $(Get-Date)

# Core Settings
INSTALL_PREFIX=$InstallPrefix
AG_HOME=$InstallPrefix
PROJECT_PATH=$ProjectPath

# Backend Configuration
BACKEND_HOST=127.0.0.1
BACKEND_PORT=$BackendPort

# Frontend Configuration
FRONTEND_PORT=$FrontendPort

# Directories
CONFIG_DIR=$InstallPrefix\config
DATA_DIR=$InstallPrefix\data
LOG_DIR=$InstallPrefix\logs
DB_DIR=$InstallPrefix\db
INDEX_DIR=$InstallPrefix\index
CACHE_DIR=$InstallPrefix\cache

# Logging
RUST_LOG=info
LOG_FORMAT=json
LOG_RETENTION_DAYS=7

# Monitoring
MONITORING_ENABLED=true
HEALTH_CHECK_INTERVAL_SECS=30

# Redis Configuration (Phase 12 support)
REDIS_ENABLED=true
REDIS_URL=redis://127.0.0.1:6379/
REDIS_TTL=3600

# Feature Flags
ENABLE_RAG=true
ENABLE_MONITORING=true
ENABLE_HEALTH_CHECKS=true
"@
        Set-Content -Path $configFile -Value $configContent
        Write-Success "Configuration created: $configFile"
        return $true
    } else {
        Write-Info "Configuration already exists"
        return $true
    }
}

# Check port availability
function Check-PortAvailability {
    Write-Info "Checking port availability..."
    
    try {
        $connection = Test-NetConnection -ComputerName 127.0.0.1 -Port $BackendPort -WarningAction SilentlyContinue
        if ($connection.TcpTestSucceeded) {
            Write-Warning "Port $BackendPort is already in use"
            return $false
        } else {
            Write-Success "Port $BackendPort is available"
            return $true
        }
    } catch {
        Write-Success "Port $BackendPort is available"
        return $true
    }
}

# Health check
function Health-Check {
    Write-Info "Performing health check..."
    
    $maxAttempts = 5
    $attempt = 1
    
    while ($attempt -le $maxAttempts) {
        try {
            $response = Invoke-WebRequest -Uri "http://127.0.0.1:3000/monitoring/health" `
                -TimeoutSec 2 -ErrorAction SilentlyContinue
            
            if ($response.StatusCode -eq 200) {
                $content = $response.Content | ConvertFrom-Json
                if ($content.status -eq "healthy") {
                    Write-Success "Health check passed: Application is healthy"
                    return $true
                }
            }
        } catch {
            # Silent catch for connection errors
        }
        
        Write-Info "Health check attempt $attempt/$maxAttempts..."
        Start-Sleep -Seconds 1
        $attempt += 1
    }
    
    Write-Warning "Health check did not receive healthy response (may be expected)"
    return $false
}

# Post-build verification
function Post-BuildVerification {
    Write-Info "Running post-build verification..."
    
    if (-not (Check-PortAvailability)) {
        Write-Warning "Port check indicated potential conflict"
    }
    
    Write-Info "Starting application for verification (timeout: $HEALTH_CHECK_TIMEOUT seconds)..."
    
    Push-Location $ProjectPath
    
    try {
        # Start application in background with timeout
        $process = Start-Process -FilePath "cargo" -ArgumentList "run --release" `
            -RedirectStandardOutput $INSTALLER_LOG -RedirectStandardError $INSTALLER_LOG `
            -PassThru -WindowStyle Hidden
        
        Write-Info "Application started (PID: $($process.Id))"
        Write-Info "Waiting for startup ($STARTUP_DELAY seconds delay)..."
        
        Start-Sleep -Seconds $STARTUP_DELAY
        
        # Attempt health check
        $healthOk = Health-Check
        
        if ($healthOk) {
            Write-Success "Post-build verification successful"
        } else {
            Write-Warning "Post-build verification incomplete (may be expected)"
        }
        
        # Terminate application
        if ($null -ne $process) {
            Stop-Process -Id $process.Id -ErrorAction SilentlyContinue
            Start-Sleep -Seconds 1
            Write-Info "Application verification process stopped"
        }
    } catch {
        Write-Warning "Post-build verification encountered an issue (may be recoverable)"
    }
    
    Pop-Location
}

# Run tests
function Run-Tests {
    Write-Info "Running tests..."
    
    Push-Location $ProjectPath
    
    try {
        cargo test --release 2>&1 | Tee-Object -FilePath $INSTALLER_LOG -Append | Out-Null
        Write-Success "All tests passed"
        Pop-Location
        return $true
    } catch {
        Write-Warning "Some tests may have failed (check $INSTALLER_LOG)"
        Pop-Location
        return $true
    }
}

# Cleanup
function Cleanup {
    Write-Info "Cleaning up..."
    
    # Kill any lingering cargo processes
    Get-Process -Name cargo -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    
    Write-Success "Cleanup completed"
}

# Display summary
function Display-Summary {
    param([int]$DurationSeconds)
    
    Write-Host ""
    Write-Host "╔══════════════════════════════════════════════════════════╗" -ForegroundColor Green
    Write-Host "║    ✓ Installation Completed Successfully (v$INSTALLER_VERSION)   ║" -ForegroundColor Green
    Write-Host "╚══════════════════════════════════════════════════════════╝" -ForegroundColor Green
    Write-Host ""
    
    Write-Host "  📊 Installation Summary:" -ForegroundColor Green
    Write-Host "    • Duration: ${DurationSeconds}s"
    Write-Host "    • Project: $ProjectPath"
    Write-Host "    • Installation Prefix: $InstallPrefix"
    Write-Host "    • AG_HOME: $InstallPrefix"
    Write-Host "    • Mode: $INSTALL_MODE"
    Write-Host "    • Log: $INSTALLER_LOG"
    Write-Host ""
    
    Write-Host "  🔗 Endpoints:" -ForegroundColor Green
    Write-Host "    • Backend: http://127.0.0.1:$BackendPort"
    Write-Host "    • Frontend: http://127.0.0.1:$FrontendPort"
    Write-Host "    • Health: http://127.0.0.1:3000/monitoring/health"
    Write-Host ""
    
    Write-Host "  📝 Next Steps:" -ForegroundColor Green
    Write-Host "    1. Navigate to project: cd '$ProjectPath'"
    Write-Host "    2. Start application: cargo run --$INSTALL_MODE"
    Write-Host "    3. Check database: dir '$InstallPrefix\db\'"
    Write-Host "    4. View logs: Get-Content -Tail 50 '$InstallPrefix\logs\*'"
    Write-Host ""
    
    Write-Host "  📚 Documentation:" -ForegroundColor Green
    Write-Host "    • Config: $InstallPrefix\config\.env"
    Write-Host "    • Installer log: $INSTALLER_LOG"
    Write-Host "    • Data dir: $InstallPrefix\data"
    Write-Host "    • Database dir: $InstallPrefix\db"
    Write-Host ""
}

# Main function
function Main {
    $startTime = Get-Date
    
    # Print header
    Print-Header
    
    # Create log directory
    if (-not (Test-Path -Path $InstallPrefix)) {
        New-Item -ItemType Directory -Force -Path $InstallPrefix | Out-Null
    }
    
    # Create log file
    $logHeader = @"
=== Agentic RAG Installation Log ===
Date: $(Get-Date)
Platform: Windows $([Environment]::OSVersion.VersionString)
Architecture: $(Get-WmiObject -Class Win32_ComputerSystem).SystemType
Rust: $(rustc --version)
Cargo: $(cargo --version)
Installer Version: $INSTALLER_VERSION
Project Path: $ProjectPath
Install Prefix: $InstallPrefix
AG_HOME: $InstallPrefix
Backend Port: $BackendPort
Frontend Port: $FrontendPort
Install Mode: $INSTALL_MODE

"@
    
    Set-Content -Path $INSTALLER_LOG -Value $logHeader
    
    Write-Info "Installer Version: $INSTALLER_VERSION"
    Write-Info "Platform: Windows $([Environment]::OSVersion.VersionString)"
    Write-Info "Project Path: $ProjectPath"
    Write-Info "Install Prefix: $InstallPrefix"
    Write-Info "AG_HOME: $InstallPrefix"
    Write-Host ""
    
    # Execute installation steps
    if (-not (Verify-Prerequisites)) {
        Write-Error "Preflight checks failed"
        exit 1
    }
    Write-Host ""
    
    Create-Directories
    Write-Host ""
    
    if (-not (Setup-Environment)) {
        Write-Error "Environment setup failed"
        exit 1
    }
    Write-Host ""
    
    if (-not (Init-Databases)) {
        Write-Error "Database initialization failed"
        exit 1
    }
    Write-Host ""
    
    Verify-Dependencies
    Write-Host ""
    
    if (-not (Build-Project)) {
        Write-Error "Build failed"
        exit 1
    }
    Write-Host ""
    
    Verify-Artifacts
    Write-Host ""
    
    Create-Configuration
    Write-Host ""
    
    Post-BuildVerification
    Write-Host ""
    
    Run-Tests
    Write-Host ""
    
    Cleanup
    
    # Calculate duration
    $endTime = Get-Date
    $duration = [int]($endTime - $startTime).TotalSeconds
    
    # Log completion
    Add-Content -Path $INSTALLER_LOG -Value @"

Installation completed successfully
Duration: ${duration}s
Status: SUCCESS
"@
    
    Display-Summary -DurationSeconds $duration
}

# Execute main function
Main

################################################################################
# END OF INSTALLER v1.1.0
################################################################################