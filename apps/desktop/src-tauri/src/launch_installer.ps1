$ErrorActionPreference = 'Stop'
try {
    # The path is data, never interpolated into PowerShell source.
    $installerProcess = Start-Process -FilePath $env:CHRONICLE_UPDATE_INSTALLER -Verb RunAs -WindowStyle Normal -PassThru -ErrorAction Stop
    if ($null -eq $installerProcess) { exit 1 }
    exit 0
} catch {
    $launchError = $_.Exception
    while ($null -ne $launchError) {
        if ($launchError -is [System.ComponentModel.Win32Exception] -and $launchError.NativeErrorCode -eq 1223) { exit 1223 }
        $launchError = $launchError.InnerException
    }
    exit 1
}
